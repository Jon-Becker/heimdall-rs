//! Credential acquisition for OrcaRouter.
//!
//! Two explicit ways to obtain an OrcaRouter API key are supported, and both collapse into the
//! same [`OrcaCredential`] so that nothing downstream — inference, model discovery, or the audit
//! log — has to know which one was used:
//!
//! * [`ApiKeyAdapter`] — the user pastes an existing `sk-orca-…` key.
//! * [`PkceAdapter`] — the user approves the request in a browser and this process redeems the
//!   one-time code with OAuth 2.0 + PKCE (RFC 7636, `S256`), never a client secret.
//!
//! Both adapters implement [`CredentialSource`]. A PKCE-issued credential is a durable API key,
//! not a refresh token: it is reused until OrcaRouter revokes it, and a relay `401` means the
//! user has to re-authorize rather than the client minting a new grant on its own.

use crate::resources::orcarouter::{
    auth_exchange_url, authorize_url, ensure_transport_security, OrcaRouterBases,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How a credential was obtained. This is metadata only; it never changes how the key is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    /// A key the user pasted into the CLI or the configuration file.
    ApiKey,
    /// A key minted by the OrcaRouter consent flow after a successful PKCE exchange.
    Pkce,
}

impl CredentialKind {
    /// The stable provider identifier used by the CLI and the configuration file.
    pub fn provider_id(self) -> &'static str {
        match self {
            CredentialKind::ApiKey => "orcarouter",
            CredentialKind::Pkce => "orcarouter-oauth",
        }
    }

    /// The human-readable label shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            CredentialKind::ApiKey => "OrcaRouter - API",
            CredentialKind::Pkce => "OrcaRouter - Auth",
        }
    }
}

impl fmt::Display for CredentialKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.provider_id())
    }
}

/// A usable OrcaRouter credential.
///
/// The key is deliberately excluded from [`fmt::Debug`] so that a credential accidentally logged
/// or printed through a struct does not leak the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct OrcaCredential {
    /// The `sk-orca-…` key. Never log this field.
    key: String,
    /// Which adapter produced it.
    pub source: CredentialKind,
    /// The OrcaRouter account the key belongs to, or an empty string when the adapter could not
    /// determine it. Used to scope `401` recovery to exactly one account.
    pub account: String,
    /// The scope the server actually granted, read from the exchange response — never the scope
    /// the client asked for.
    pub scope: String,
    /// Monotonic credential generation, assigned by [`CredentialStore`].
    pub generation: u64,
}

impl OrcaCredential {
    /// Wraps an already-known key.
    pub fn new(key: impl Into<String>, source: CredentialKind) -> Self {
        OrcaCredential {
            key: key.into(),
            source,
            account: String::new(),
            scope: "api".to_string(),
            generation: 0,
        }
    }

    /// Returns the raw key. Callers must not log, print, or persist the result anywhere other than
    /// the project's existing credential store.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns a value safe to show in a status line or a log.
    pub fn masked_key(&self) -> String {
        mask_key(&self.key)
    }

    /// Whether the granted scope permits inference against the relay.
    pub fn allows_inference(&self) -> bool {
        scope_allows_inference(&self.scope)
    }
}

impl fmt::Debug for OrcaCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrcaCredential")
            .field("key", &self.masked_key())
            .field("source", &self.source)
            .field("account", &self.account)
            .field("scope", &self.scope)
            .field("generation", &self.generation)
            .finish()
    }
}

/// Masks a key for display: keeps the `sk-orca-` prefix and at most four trailing characters.
pub fn mask_key(key: &str) -> String {
    const MASK: &str = "…";
    if key.is_empty() {
        return String::new();
    }
    let (prefix, rest) = match key.split_once('-') {
        Some(("sk", rest)) => ("sk-", rest),
        _ => ("", key),
    };
    let (provider, tail) = match rest.split_once('-') {
        Some((provider, tail)) if !provider.is_empty() => {
            (format!("{prefix}{provider}-"), tail)
        }
        _ => (prefix.to_string(), rest),
    };
    let visible: String = tail.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{provider}{MASK}{visible}")
}

/// Whether a granted scope is enough to call the inference API.
pub fn scope_allows_inference(scope: &str) -> bool {
    scope.split([' ', ',']).any(|part| part == "api")
}

/// A cheap structural check on user input. A well-formed prefix is not proof that a key is valid;
/// only the first real request establishes that.
pub fn looks_like_api_key(key: &str) -> bool {
    let key = key.trim();
    key.starts_with("sk-orca-") && key.len() > "sk-orca-".len()
}

/// Failures that can occur while acquiring a credential.
///
/// No variant carries a verifier, an auth code, or a key, so these values can be logged and
/// rendered verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No credential source was configured.
    NotConfigured,
    /// The pasted key does not look like an OrcaRouter key.
    MalformedKey,
    /// The user declined the authorization request.
    Denied,
    /// The `state` returned by the browser did not match the one this process generated.
    StateMismatch,
    /// The authorization window closed before a code arrived.
    TimedOut,
    /// The user or the host cancelled the attempt.
    Cancelled,
    /// The exchange endpoint answered with an unexpected status.
    ExchangeRejected {
        /// HTTP status returned by the exchange endpoint.
        status: u16,
        /// A short, sanitised explanation.
        reason: String,
    },
    /// The granted scope does not permit inference.
    InsufficientScope {
        /// The scope the server actually granted.
        granted: String,
    },
    /// A request to the auth origin could not be completed.
    Network(String),
    /// The exchange response was not understood.
    Protocol(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::NotConfigured => write!(
                f,
                "no OrcaRouter credential configured. paste a key with `heimdall config orcarouter_api_key <KEY>` or run `heimdall orcarouter login`."
            ),
            AuthError::MalformedKey => {
                write!(f, "that does not look like an OrcaRouter key (expected `sk-orca-…`).")
            }
            AuthError::Denied => write!(f, "authorization was denied in the browser."),
            AuthError::StateMismatch => write!(
                f,
                "the authorization response did not match this login attempt. start the login again."
            ),
            AuthError::TimedOut => {
                write!(f, "the authorization window closed before a code arrived. try again.")
            }
            AuthError::Cancelled => write!(f, "login cancelled."),
            AuthError::ExchangeRejected { status, reason } => {
                write!(f, "the OrcaRouter exchange endpoint rejected the code ({status}): {reason}")
            }
            AuthError::InsufficientScope { granted } => write!(
                f,
                "the granted scope is `{granted}`, which does not permit inference. ask a workspace owner to approve `api` access."
            ),
            AuthError::Network(e) => write!(f, "could not reach the OrcaRouter auth origin: {e}"),
            AuthError::Protocol(e) => write!(f, "unexpected response from the auth origin: {e}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// The single seam both authentication choices implement.
pub trait CredentialSource: Send + Sync {
    /// The kind of credential this source produces.
    fn kind(&self) -> CredentialKind;

    /// Obtain a credential, or fail without leaking anything the user typed.
    fn acquire(&self) -> Result<OrcaCredential, AuthError>;
}

/// The API-key adapter: wraps a key the user already owns.
#[derive(Debug, Clone)]
pub struct ApiKeyAdapter {
    key: String,
}

impl ApiKeyAdapter {
    /// Creates an adapter around a user-supplied key.
    pub fn new(key: impl Into<String>) -> Self {
        ApiKeyAdapter { key: key.into() }
    }
}

impl CredentialSource for ApiKeyAdapter {
    fn kind(&self) -> CredentialKind {
        CredentialKind::ApiKey
    }

    fn acquire(&self) -> Result<OrcaCredential, AuthError> {
        let key = self.key.trim();
        if key.is_empty() {
            return Err(AuthError::NotConfigured);
        }
        if !looks_like_api_key(key) {
            return Err(AuthError::MalformedKey);
        }
        Ok(OrcaCredential::new(key, CredentialKind::ApiKey))
    }
}

/// Opens the authorization URL in a browser. Injectable so tests never launch one.
pub type BrowserOpener = Arc<dyn Fn(&str) + Send + Sync>;

/// The PKCE adapter.
///
/// Uses Flow A (loopback redirect): the process listens on an ephemeral `127.0.0.1` port, opens
/// the consent screen, and receives the one-time code on the redirect. `S256` is always sent
/// because the consent screen lets the user choose to display a code instead.
pub struct PkceAdapter {
    bases: OrcaRouterBases,
    app_name: String,
    opener: BrowserOpener,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
}

impl fmt::Debug for PkceAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PkceAdapter")
            .field("bases", &self.bases)
            .field("app_name", &self.app_name)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl PkceAdapter {
    /// Builds an adapter for the given origins, app label, and browser opener.
    pub fn new(
        bases: OrcaRouterBases,
        app_name: impl Into<String>,
        opener: BrowserOpener,
        timeout: Duration,
    ) -> Self {
        PkceAdapter {
            bases,
            app_name: app_name.into(),
            opener,
            timeout,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns a handle that can cancel an in-flight login from another thread.
    pub fn cancellation_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    /// Drives the full loopback flow and returns the redeemed credential.
    pub fn run(&self) -> Result<OrcaCredential, AuthError> {
        ensure_transport_security(&self.bases.auth)?;

        let verifier = generate_verifier();
        let challenge = code_challenge(&verifier);
        let state = generate_state();

        // Listen before opening the browser so the callback port is known and nothing races.
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| AuthError::Network(format!("could not bind loopback listener: {e}")))?;
        listener.set_nonblocking(true).map_err(|e| {
            AuthError::Network(format!("could not configure loopback listener: {e}"))
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| AuthError::Network(format!("could not read listener address: {e}")))?
            .port();
        let callback_url = format!("http://127.0.0.1:{port}/cb");

        let url = authorize_url(
            &self.bases,
            &callback_url,
            &challenge,
            "S256",
            &state,
            &self.app_name,
            "api",
        );
        (self.opener)(&url);

        let (code, returned_state) = self.wait_for_callback(&listener)?;

        // The state check is constant time and happens before the code is used for anything.
        if !constant_time_eq(returned_state.as_bytes(), state.as_bytes()) {
            return Err(AuthError::StateMismatch);
        }

        self.exchange(&code, &verifier)
    }

    /// Waits for the browser to hit the loopback callback, honouring the timeout and the
    /// cancellation flag without spinning.
    fn wait_for_callback(
        &self,
        listener: &TcpListener,
    ) -> Result<(String, String), AuthError> {
        let deadline = std::time::Instant::now() + self.timeout;
        loop {
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(AuthError::Cancelled);
            }
            if std::time::Instant::now() >= deadline {
                return Err(AuthError::TimedOut);
            }
            match listener.accept() {
                Ok((stream, _)) => return handle_callback(stream),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(AuthError::Network(format!("loopback listener failed: {e}"))),
            }
        }
    }

    /// Redeems the one-time code at the auth origin for a durable API key.
    fn exchange(&self, code: &str, verifier: &str) -> Result<OrcaCredential, AuthError> {
        let url = auth_exchange_url(&self.bases);
        let body = serde_json::json!({
            "code": code,
            "code_verifier": verifier,
            "code_challenge_method": "S256",
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AuthError::Network(e.to_string()))?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| AuthError::Network(e.to_string()))?;

        let status = response.status().as_u16();
        let text = response.text().unwrap_or_default();

        if status != 200 {
            // The auth origin answers with `{"error": "...", "error_description": "..."}` for
            // OAuth failures; never echo anything else verbatim.
            let reason = serde_json::from_str::<ExchangeError>(&text)
                .ok()
                .and_then(|e| e.error.or(e.error_description))
                .unwrap_or_else(|| exchange_reason(status));
            return Err(AuthError::ExchangeRejected { status, reason: sanitize(&reason) });
        }

        let parsed: ExchangeSuccess = serde_json::from_str(&text)
            .map_err(|_| AuthError::Protocol("exchange response was not valid JSON".to_string()))?;

        if !looks_like_api_key(&parsed.key) {
            return Err(AuthError::Protocol(
                "exchange response did not contain an OrcaRouter key".to_string(),
            ));
        }

        // Read the granted scope back. It is what was approved, not what was requested.
        if !scope_allows_inference(&parsed.scope) {
            return Err(AuthError::InsufficientScope { granted: parsed.scope });
        }

        Ok(OrcaCredential {
            key: parsed.key,
            source: CredentialKind::Pkce,
            account: parsed.user_id.unwrap_or_default(),
            scope: parsed.scope,
            generation: 0,
        })
    }
}

impl CredentialSource for PkceAdapter {
    fn kind(&self) -> CredentialKind {
        CredentialKind::Pkce
    }

    fn acquire(&self) -> Result<OrcaCredential, AuthError> {
        self.run()
    }
}

#[derive(Deserialize)]
struct ExchangeSuccess {
    #[serde(default)]
    key: String,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default = "default_scope")]
    scope: String,
}

fn default_scope() -> String {
    "api".to_string()
}

#[derive(Deserialize)]
struct ExchangeError {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

fn exchange_reason(status: u16) -> String {
    match status {
        400 => "the authorization request did not match the code challenge".to_string(),
        403 => "the code is unknown, expired, already used, or the verifier did not match".to_string(),
        429 => "too many authorization requests; wait before trying again".to_string(),
        _ => "the exchange failed".to_string(),
    }
}

/// Strips anything that could be a credential from a server-supplied message before it reaches a
/// terminal.
fn sanitize(message: &str) -> String {
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
    out.chars().take(200).collect()
}

/// Serves the browser a closing page and extracts `code`/`state` from the request line.
fn handle_callback(mut stream: TcpStream) -> Result<(String, String), AuthError> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| AuthError::Network(format!("could not configure callback socket: {e}")))?;

    let mut buffer = [0u8; 4096];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default()
        .to_string();

    let body = "<!doctype html><meta charset=\"utf-8\"><title>OrcaRouter</title>\
                <p>Connected. You can close this tab and return to heimdall.</p>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    let parsed = parse_callback_target(&target)?;
    Ok(parsed)
}

/// Parses `code`/`state` out of the callback path, mapping a denial to [`AuthError::Denied`].
fn parse_callback_target(target: &str) -> Result<(String, String), AuthError> {
    let (_, query) = match target.split_once('?') {
        Some(parts) => parts,
        None => return Err(AuthError::Protocol("callback had no query".to_string())),
    };

    let mut code = None;
    let mut state = String::new();
    let mut error = None;
    for pair in query.split('&') {
        let (key, value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => continue,
        };
        let value = percent_decode(value);
        match key {
            "code" => code = Some(value),
            "state" => state = value,
            "error" => error = Some(value),
            _ => {}
        }
    }

    if let Some(error) = error {
        return Err(if error == "access_denied" {
            AuthError::Denied
        } else {
            AuthError::ExchangeRejected { status: 400, reason: sanitize(&error) }
        });
    }

    match code {
        Some(code) if !code.is_empty() => Ok((code, state)),
        _ => Err(AuthError::Protocol("callback carried no code".to_string())),
    }
}

/// Minimal percent-decoding for `application/x-www-form-urlencoded` query values.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or_default();
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Compares two byte strings without short-circuiting.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Generates a fresh PKCE verifier from a cryptographic RNG.
pub fn generate_verifier() -> String {
    base64url(&random_bytes(32))
}

/// Generates a fresh opaque `state` value from a cryptographic RNG.
pub fn generate_state() -> String {
    base64url(&random_bytes(16))
}

/// Computes `base64url(sha256(verifier))` without padding, as required for `S256`.
pub fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64url(&digest)
}

fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut buffer = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buffer);
    buffer
}

/// Unpadded base64url encoding, as required by RFC 7636.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 0x3f] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 0x3f] as char);
        }
    }
    out
}

/// Tracks credential generations so a stale asynchronous result can never be written over a newer
/// login.
#[derive(Debug, Default)]
pub struct CredentialStore {
    generation: AtomicU64,
}

impl CredentialStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        CredentialStore { generation: AtomicU64::new(0) }
    }

    /// Starts a new login attempt and returns its generation.
    pub fn begin_attempt(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Returns the newest generation.
    pub fn current(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Stamps a fresh credential with the current generation. Returns `None` when the attempt is
    /// stale, in which case the caller must discard the credential and write nothing.
    pub fn commit(&self, attempt: u64, mut credential: OrcaCredential) -> Option<OrcaCredential> {
        if attempt != self.current() {
            return None;
        }
        credential.generation = attempt;
        Some(credential)
    }
}
