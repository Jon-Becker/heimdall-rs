//! OrcaRouter provider integration.
//!
//! OrcaRouter is an OpenAI-compatible AI gateway: one endpoint routes to many model providers.
//! This module is the named provider entry for it, mirroring [`crate::resources::openrouter`] but
//! keeping OrcaRouter's own configuration, base URLs, credential handling, and model catalog.
//!
//! Inference and model discovery live on `https://api.orcarouter.ai/v1`. Authentication lives on
//! a different origin, `https://www.orcarouter.ai`, and its endpoints are under `/api/v1/auth`.
//! The two origins are configured independently; neither is derived from the other.

pub mod auth;
pub mod catalog;

pub use auth::{
    base64url, code_challenge, generate_state, generate_verifier, mask_key, scope_allows_inference,
    ApiKeyAdapter, AuthError, CredentialKind, CredentialSource, CredentialStore, OrcaCredential,
    PkceAdapter,
};
pub use catalog::{
    discover_models, fallback_models, Capability, CatalogStatus, ModelInfo, ModelSelector,
    SelectedModel,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::debug;

/// The public OrcaRouter authentication origin.
pub const DEFAULT_AUTH_BASE: &str = "https://www.orcarouter.ai";

/// The public OrcaRouter inference origin, including the `/v1` prefix the relay expects.
pub const DEFAULT_API_BASE: &str = "https://api.orcarouter.ai/v1";

/// The recommended model for this toolkit.
///
/// OrcaCyber Zero is the cyber-specialised route: it is intended for security teams whose use has
/// been validated through OrcaRouter's business verification, and authorized security work is
/// permitted there where a general-purpose gateway commonly refuses it. The cyber route has to be
/// unlocked for the workspace before it answers; [`FALLBACK_MODEL`] is used until then.
pub const DEFAULT_MODEL: &str = "orcacyber-zero";

/// The general-purpose route used when the cyber route is not unlocked for the workspace.
pub const FALLBACK_MODEL: &str = "orcarouter/fusion";

/// The key-management page where a user can review or revoke keys issued to this app.
pub const KEY_DASHBOARD_URL: &str = "https://www.orcarouter.ai/console/authorized-apps";

/// Upper bound on a single inference request.
const CHAT_TIMEOUT: Duration = Duration::from_secs(120);

/// Default time the connect flow waits for the browser to come back.
pub const AUTH_TIMEOUT_DEFAULT: Duration = Duration::from_secs(300);

/// The two OrcaRouter origins, kept explicit rather than derived from one another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrcaRouterBases {
    /// Origin that serves the consent screen and the code exchange, e.g. `https://www.orcarouter.ai`.
    pub auth: String,
    /// Origin that serves inference and the model catalog, e.g. `https://api.orcarouter.ai/v1`.
    pub api: String,
}

impl Default for OrcaRouterBases {
    fn default() -> Self {
        OrcaRouterBases {
            auth: DEFAULT_AUTH_BASE.to_string(),
            api: DEFAULT_API_BASE.to_string(),
        }
    }
}

impl OrcaRouterBases {
    /// Resolves the origins.
    ///
    /// Precedence, highest first: an explicit value (CLI flag), `ORCA_AUTH_BASE_URL` /
    /// `ORCA_API_BASE_URL`, the shared self-hosted `ORCA_BASE_URL`, then the public defaults. A
    /// shared base applies `/v1` to the inference origin only; it is never used to derive the auth
    /// origin from the inference origin or the other way round.
    pub fn resolve(
        explicit_auth: Option<&str>,
        explicit_api: Option<&str>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        let shared = env("ORCA_BASE_URL").filter(|v| !v.trim().is_empty());
        let auth = explicit_auth
            .map(str::to_string)
            .filter(|v| !v.trim().is_empty())
            .or_else(|| env("ORCA_AUTH_BASE_URL").filter(|v| !v.trim().is_empty()))
            .or_else(|| shared.clone())
            .unwrap_or_else(|| DEFAULT_AUTH_BASE.to_string());

        let api = explicit_api
            .map(str::to_string)
            .filter(|v| !v.trim().is_empty())
            .or_else(|| env("ORCA_API_BASE_URL").filter(|v| !v.trim().is_empty()))
            .or_else(|| shared.map(|base| format!("{}/v1", base.trim_end_matches('/'))))
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());

        OrcaRouterBases { auth: auth.trim_end_matches('/').to_string(), api }
    }

    /// Resolves from the process environment.
    pub fn from_env() -> Self {
        OrcaRouterBases::resolve(None, None, &|name| std::env::var(name).ok())
    }

    /// Resolves using explicit values, falling back to the process environment.
    pub fn with_overrides(auth: Option<&str>, api: Option<&str>) -> Self {
        OrcaRouterBases::resolve(auth, api, &|name| std::env::var(name).ok())
    }
}

/// Rejects plaintext transports for anything that is not a loopback address.
pub fn ensure_transport_security(base: &str) -> Result<(), AuthError> {
    let parsed = reqwest::Url::parse(base)
        .map_err(|_| AuthError::Network(format!("`{base}` is not a valid URL")))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or_default();
            if is_loopback_host(host) {
                Ok(())
            } else {
                Err(AuthError::Network(format!(
                    "`{base}` uses plaintext HTTP; only loopback origins may do that"
                )))
            }
        }
        other => Err(AuthError::Network(format!("`{other}` is not a supported URL scheme"))),
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host == "localhost" || host == "::1" || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
}

/// Builds the consent URL. `/auth` is fixed; only the origin is configurable.
pub fn authorize_url(
    bases: &OrcaRouterBases,
    callback_url: &str,
    challenge: &str,
    method: &str,
    state: &str,
    app_name: &str,
    scope: &str,
) -> String {
    format!(
        "{}/auth?callback_url={}&code_challenge={}&code_challenge_method={}&state={}&app_name={}&scope={}",
        bases.auth.trim_end_matches('/'),
        encode_component(callback_url),
        encode_component(challenge),
        encode_component(method),
        encode_component(state),
        encode_component(app_name),
        encode_component(scope),
    )
}

/// Builds the code-exchange URL. It always lives on the auth origin under `/api/v1/auth/keys` —
/// never on the inference origin, which answers 404 for that path.
pub fn auth_exchange_url(bases: &OrcaRouterBases) -> String {
    format!("{}/api/v1/auth/keys", bases.auth.trim_end_matches('/'))
}

/// Builds the model-catalog URL on the inference origin.
pub fn models_url(bases: &OrcaRouterBases, capability: Option<Capability>) -> String {
    match capability {
        Some(capability) if capability.query_value().is_some() => {
            format!("{}/models?capability={}", bases.api.trim_end_matches('/'), capability.query_value().unwrap_or_default())
        }
        _ => format!("{}/models", bases.api.trim_end_matches('/')),
    }
}

/// Percent-encodes a query component, keeping only RFC 3986 unreserved characters.
pub fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Failure modes of an OrcaRouter inference or catalog call.
///
/// The variants carry no credential material, so they are safe to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// No credential was configured.
    MissingCredential,
    /// The relay rejected the key; the account has to re-authorize.
    Unauthorized,
    /// The workspace is not entitled to the requested model.
    ModelNotEntitled {
        /// The model the relay refused.
        model: String,
        /// A short, sanitised explanation supplied by the relay.
        detail: String,
    },
    /// The request exceeded a rate limit.
    RateLimited(Option<u64>),
    /// The request or response could not be completed.
    Network(String),
    /// The response was not the OpenAI-compatible shape expected.
    Protocol(String),
    /// The model catalog could not be read.
    Catalog(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::MissingCredential => write!(f, "no OrcaRouter credential configured"),
            ProviderError::Unauthorized => write!(
                f,
                "OrcaRouter rejected the key (401). run `heimdall orcarouter login` to re-authorize."
            ),
            ProviderError::ModelNotEntitled { model, detail } => write!(
                f,
                "the workspace is not entitled to `{model}` (403){}. pick another model with `--model`, or use `{FALLBACK_MODEL}`.",
                if detail.is_empty() { String::new() } else { format!(": {detail}") }
            ),
            ProviderError::RateLimited(None) => write!(f, "OrcaRouter rate limit reached (429)"),
            ProviderError::RateLimited(Some(seconds)) => {
                write!(f, "OrcaRouter rate limit reached (429); retry after {seconds}s")
            }
            ProviderError::Network(e) => write!(f, "could not reach OrcaRouter: {e}"),
            ProviderError::Protocol(e) => write!(f, "unexpected OrcaRouter response: {e}"),
            ProviderError::Catalog(e) => write!(f, "could not read the OrcaRouter model catalog: {e}"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<AuthError> for ProviderError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::NotConfigured | AuthError::MalformedKey => ProviderError::MissingCredential,
            AuthError::Network(message) => ProviderError::Network(message),
            other => ProviderError::Protocol(other.to_string()),
        }
    }
}

/// Maps an HTTP status from the relay onto a [`ProviderError`].
pub fn classify_status(status: u16, model: &str, body: &str) -> Option<ProviderError> {
    match status {
        200..=299 => None,
        401 => Some(ProviderError::Unauthorized),
        403 => Some(ProviderError::ModelNotEntitled {
            model: model.to_string(),
            detail: server_message(body),
        }),
        429 => Some(ProviderError::RateLimited(parse_retry_after(body))),
        _ => {
            let detail = serde_json::from_str::<ApiErrorEnvelope>(body)
                .ok()
                .and_then(|envelope| envelope.error.map(|e| e.message))
                .map(|message| sanitize(&message))
                .unwrap_or_else(|| sanitize(body));
            Some(ProviderError::Protocol(format!("HTTP {status}: {detail}")))
        }
    }
}

/// Extracts a short, sanitised message from an OpenAI-style error envelope.
fn server_message(body: &str) -> String {
    serde_json::from_str::<ApiErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error)
        .map(|error| sanitize(&error.message))
        .filter(|message| !message.is_empty())
        .unwrap_or_default()
}

fn parse_retry_after(body: &str) -> Option<u64> {    serde_json::from_str::<ApiErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error)
        .and_then(|error| error.retry_after)
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    #[serde(default)]
    error: Option<ApiErrorBody>,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    retry_after: Option<u64>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: String,
    json_schema: JsonSchemaFormat,
}

#[derive(Serialize)]
struct JsonSchemaFormat {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

fn build_client(timeout: Duration) -> Result<reqwest::blocking::Client, ProviderError> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))
}

fn post_chat(
    bases: &OrcaRouterBases,
    api_key: &str,
    request: &ChatRequest<'_>,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingCredential);
    }
    ensure_transport_security(&bases.api).map_err(|e| ProviderError::Network(e.to_string()))?;

    let url = format!("{}/chat/completions", bases.api.trim_end_matches('/'));
    let client = build_client(CHAT_TIMEOUT)?;
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(request)
        .send()
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();

    if let Some(error) = classify_status(status, request.model, &body) {
        return Err(error);
    }

    let parsed: ChatResponse = serde_json::from_str(&body)
        .map_err(|_| ProviderError::Protocol("response was not valid JSON".to_string()))?;

    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .ok_or_else(|| ProviderError::Protocol("response carried no message content".to_string()))
}

/// Completes a plain text prompt against OrcaRouter.
///
/// `model` may be empty, in which case [`DEFAULT_MODEL`] is used.
pub fn complete_chat(
    prompt: &str,
    api_key: &str,
    model: &str,
    bases: &OrcaRouterBases,
) -> Result<String, ProviderError> {
    let model = if model.trim().is_empty() { DEFAULT_MODEL } else { model.trim() };
    debug!("completing chat via OrcaRouter with model '{model}'");

    let request = ChatRequest {
        model,
        messages: vec![ChatMessage { role: "user", content: prompt, name: "user" }],
        response_format: None,
    };
    post_chat(bases, api_key, &request)
}

/// Completes a prompt and parses the reply as a JSON object matching `schema`.
pub fn complete_chat_structured<T: DeserializeOwned>(
    prompt: &str,
    api_key: &str,
    model: &str,
    schema_name: &str,
    schema: serde_json::Value,
    bases: &OrcaRouterBases,
) -> Result<T, ProviderError> {
    let model = if model.trim().is_empty() { DEFAULT_MODEL } else { model.trim() };
    debug!("completing structured chat via OrcaRouter with model '{model}'");

    let request = ChatRequest {
        model,
        messages: vec![ChatMessage { role: "user", content: prompt, name: "user" }],
        response_format: Some(ResponseFormat {
            kind: "json_schema".to_string(),
            json_schema: JsonSchemaFormat {
                name: schema_name.to_string(),
                description: Some("Annotated contract source code".to_string()),
                schema,
                strict: Some(true),
            },
        }),
    };

    let body = post_chat(bases, api_key, &request)?;
    serde_json::from_str::<T>(&body)
        .map_err(|_| ProviderError::Protocol("structured response did not match the schema".to_string()))
}

/// Strips credential-shaped tokens from server-supplied text.
pub(crate) fn sanitize(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for token in message.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        if token.starts_with("sk-orca-") || token.starts_with("sk-") {
            out.push_str("[redacted]");
        } else {
            out.push_str(token);
        }
    }
    out.chars().take(300).collect()
}

/// Which model provider an AI-backed command should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProvider {
    /// The pre-existing OpenRouter integration.
    #[default]
    OpenRouter,
    /// The OrcaRouter gateway.
    OrcaRouter,
}

impl LlmProvider {
    /// Parses the `--provider` value.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "openrouter" => Some(LlmProvider::OpenRouter),
            "orcarouter" | "orca" => Some(LlmProvider::OrcaRouter),
            _ => None,
        }
    }

    /// The stable identifier used on the command line.
    pub fn id(self) -> &'static str {
        match self {
            LlmProvider::OpenRouter => "openrouter",
            LlmProvider::OrcaRouter => "orcarouter",
        }
    }
}

impl fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}
