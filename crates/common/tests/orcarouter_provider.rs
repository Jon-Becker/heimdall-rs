//! Provider-level tests for OrcaRouter: origins, transport policy, error classification, and the
//! wire contract of the implemented request paths.
//!
//! The live test at the end runs only when `ORCAROUTER_API_KEY` is present and exercises the same
//! code paths the CLI uses.

use heimdall_common::resources::llm::LlmSettings;
use heimdall_common::resources::orcarouter::{
    classify_status, complete_chat, discover_models, ensure_transport_security, models_url,
    LlmProvider, OrcaRouterBases, ProviderError, DEFAULT_API_BASE, DEFAULT_AUTH_BASE, DEFAULT_MODEL,
    FALLBACK_MODEL,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A recorded request to the fake relay.
#[derive(Debug, Clone)]
struct Recorded {
    path: String,
    authorization: Option<String>,
    body: String,
}

/// A fake OpenAI-compatible relay whose answers are decided per request.
struct FakeRelay {
    base: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl FakeRelay {
    fn start(json_response: &'static str) -> Self {
        FakeRelay::with_responder(move |_, _| (200, json_response.to_string()))
    }

    /// Starts a relay that answers through `responder(path, body)`.
    fn with_responder(responder: impl Fn(&str, &str) -> (u16, String) + Send + 'static) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fake relay");
        let port = listener.local_addr().expect("addr").port();
        let requests: Arc<Mutex<Vec<Recorded>>> = Arc::new(Mutex::new(Vec::new()));
        let requests_thread = Arc::clone(&requests);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                handle(stream, &responder, &requests_thread);
            }
        });

        FakeRelay { base: format!("http://127.0.0.1:{port}/v1"), requests }
    }

    fn bases(&self) -> OrcaRouterBases {
        OrcaRouterBases { auth: self.base.trim_end_matches("/v1").to_string(), api: self.base.clone() }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().expect("requests").clone()
    }
}

fn handle(
    mut stream: TcpStream,
    responder: &dyn Fn(&str, &str) -> (u16, String),
    requests: &Arc<Mutex<Vec<Recorded>>>,
) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break buffer.len();
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let path = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default()
        .to_string();
    let authorization = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization").then(|| value.trim().to_string())
    });
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }

    let body = String::from_utf8_lossy(&body).to_string();
    requests
        .lock()
        .expect("requests")
        .push(Recorded { path: path.clone(), authorization, body: body.clone() });

    let (status, response) = responder(&path, &body);
    let reply = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    );
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.flush();
}

fn env_lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> =
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
    move |name: &str| {
        owned.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone())
    }
}

// ---------------------------------------------------------------------------
// Origins
// ---------------------------------------------------------------------------

#[test]
fn defaults_keep_auth_and_inference_on_separate_origins() {
    let bases = OrcaRouterBases::resolve(None, None, &env_lookup(&[]));
    assert_eq!(bases.auth, DEFAULT_AUTH_BASE);
    assert_eq!(bases.api, DEFAULT_API_BASE);
    assert_eq!(bases.api, "https://api.orcarouter.ai/v1");
    assert_eq!(bases.auth, "https://www.orcarouter.ai");
    assert!(!bases.api.contains("www.orcarouter.ai"));
    assert!(!bases.auth.ends_with("/v1"));
}

#[test]
fn explicit_overrides_win_over_every_fallback() {
    let bases = OrcaRouterBases::resolve(
        Some("https://auth.example.test"),
        Some("https://relay.example.test/v1"),
        &env_lookup(&[
            ("ORCA_BASE_URL", "https://shared.example.test"),
            ("ORCA_AUTH_BASE_URL", "https://env-auth.example.test"),
            ("ORCA_API_BASE_URL", "https://env-api.example.test"),
        ]),
    );
    assert_eq!(bases.auth, "https://auth.example.test");
    assert_eq!(bases.api, "https://relay.example.test/v1");
}

#[test]
fn a_shared_self_hosted_base_applies_to_both_origins() {
    let bases = OrcaRouterBases::resolve(
        None,
        None,
        &env_lookup(&[("ORCA_BASE_URL", "https://orca.internal.example")]),
    );
    assert_eq!(bases.auth, "https://orca.internal.example");
    assert_eq!(bases.api, "https://orca.internal.example/v1");

    let bases = OrcaRouterBases::resolve(
        None,
        None,
        &env_lookup(&[
            ("ORCA_BASE_URL", "https://orca.internal.example"),
            ("ORCA_API_BASE_URL", "https://relay.internal.example/v1"),
        ]),
    );
    assert_eq!(bases.auth, "https://orca.internal.example");
    assert_eq!(bases.api, "https://relay.internal.example/v1");
}

#[test]
fn plaintext_http_is_only_allowed_for_loopback() {
    assert!(ensure_transport_security("https://www.orcarouter.ai").is_ok());
    assert!(ensure_transport_security("http://127.0.0.1:8080").is_ok());
    assert!(ensure_transport_security("http://localhost:8080").is_ok());
    assert!(ensure_transport_security("http://[::1]:8080").is_ok());
    assert!(ensure_transport_security("http://orca.example.test").is_err());
    assert!(ensure_transport_security("ftp://orca.example.test").is_err());
}

#[test]
fn model_catalog_url_uses_the_inference_origin() {
    let bases = OrcaRouterBases::default();
    assert_eq!(models_url(&bases, None), "https://api.orcarouter.ai/v1/models");
    assert_eq!(
        models_url(&bases, Some(heimdall_common::resources::orcarouter::Capability::Chat)),
        "https://api.orcarouter.ai/v1/models?capability=chat"
    );
}

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

#[test]
fn status_codes_map_to_actionable_errors() {
    assert_eq!(classify_status(200, "m", ""), None);
    assert_eq!(classify_status(401, "m", ""), Some(ProviderError::Unauthorized));
    assert_eq!(
        classify_status(403, "orcacyber-zero", ""),
        Some(ProviderError::ModelNotEntitled {
            model: "orcacyber-zero".to_string(),
            detail: String::new()
        })
    );
    assert_eq!(classify_status(429, "m", ""), Some(ProviderError::RateLimited(None)));
    assert!(matches!(classify_status(500, "m", "boom"), Some(ProviderError::Protocol(_))));
}

#[test]
fn errors_never_echo_the_credential() {
    let body = r#"{"error":{"message":"bad key sk-orca-secret-value-1234"}}"#;
    let error = classify_status(400, "m", body).expect("error");
    assert!(!error.to_string().contains("sk-orca-secret-value-1234"), "{error}");
    assert!(error.to_string().contains("[redacted]"));
}

// ---------------------------------------------------------------------------
// Wire contract of the implemented path
// ---------------------------------------------------------------------------

#[test]
fn chat_requests_go_to_the_inference_origin_with_bearer_auth() {
    let relay = FakeRelay::start(r#"{"choices":[{"message":{"content":"hello"}}]}"#);
    let bases = relay.bases();

    let reply = complete_chat("say hi", "sk-orca-unit-test-key", "openai/gpt-5.5", &bases)
        .expect("request should succeed");
    assert_eq!(reply, "hello");

    let requests = relay.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer sk-orca-unit-test-key")
    );
    assert!(requests[0].body.contains("\"model\":\"openai/gpt-5.5\""));
}

#[test]
fn model_discovery_uses_the_models_endpoint_with_capability_filter() {
    let relay = FakeRelay::start(
        r#"{"data":[{"id":"openai/gpt-5.5","supported_endpoint_types":["openai"],"architecture":{"input_modalities":["text"]}}]}"#,
    );
    let bases = relay.bases();

    let models = discover_models(&bases, "sk-orca-unit-test-key", heimdall_common::resources::orcarouter::Capability::Chat)
        .expect("catalog should be readable");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "openai/gpt-5.5");

    let requests = relay.requests();
    assert_eq!(requests[0].path, "/v1/models?capability=chat");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer sk-orca-unit-test-key"),
        "discovery authenticates with the same credential as inference"
    );
}

#[test]
fn a_missing_credential_is_refused_before_any_request() {
    let bases = OrcaRouterBases::default();
    assert_eq!(
        complete_chat("hi", "", "openai/gpt-5.5", &bases),
        Err(ProviderError::MissingCredential)
    );
    assert_eq!(
        discover_models(&bases, "", heimdall_common::resources::orcarouter::Capability::Chat),
        Err(ProviderError::MissingCredential)
    );
}

#[test]
fn provider_ids_are_stable_and_distinct() {
    assert_eq!(LlmProvider::parse("openrouter"), Some(LlmProvider::OpenRouter));
    assert_eq!(LlmProvider::parse("orcarouter"), Some(LlmProvider::OrcaRouter));
    assert_eq!(LlmProvider::parse(""), Some(LlmProvider::OpenRouter));
    assert_eq!(LlmProvider::parse("nope"), None);

    assert_eq!(LlmProvider::OrcaRouter.id(), "orcarouter");
    assert_ne!(LlmProvider::OrcaRouter.id(), LlmProvider::OpenRouter.id());
}

// ---------------------------------------------------------------------------
// Model entitlement
// ---------------------------------------------------------------------------

const TWO_MODEL_CATALOG: &str = r#"{"data":[
    {"id":"blocked/model","architecture":{"input_modalities":["text"]},"supported_endpoint_types":["openai"]},
    {"id":"good/model","architecture":{"input_modalities":["text"]},"supported_endpoint_types":["openai"]}
]}"#;

fn entitlement_relay(refuse_good_model: bool) -> FakeRelay {
    FakeRelay::with_responder(move |path, body| {
        if path.starts_with("/v1/models") {
            return (200, TWO_MODEL_CATALOG.to_string());
        }
        let refused = body.contains("blocked/model")
            || (refuse_good_model && body.contains("good/model"));
        if refused {
            (403, r#"{"error":{"message":"this key has no access to that model"}}"#.to_string())
        } else {
            (200, r#"{"choices":[{"message":{"content":"hello"}}]}"#.to_string())
        }
    })
}

fn chat_requests(relay: &FakeRelay) -> Vec<Recorded> {
    relay
        .requests()
        .into_iter()
        .filter(|request| request.path == "/v1/chat/completions")
        .collect()
}

#[tokio::test]
async fn a_model_the_key_cannot_use_falls_back_to_a_catalog_model() {
    let relay = entitlement_relay(false);
    let settings = LlmSettings::new(
        LlmProvider::OrcaRouter,
        "sk-orca-unit-test-key",
        "blocked/model",
        relay.bases(),
    );

    let reply = settings
        .complete_text("hi".to_string(), "blocked/model".to_string())
        .await
        .expect("the fallback model should answer");

    assert_eq!(reply, "hello");
    let requests = chat_requests(&relay);
    assert_eq!(requests.len(), 2, "the refused model is retried once with another catalog model");
    assert!(requests[0].body.contains("blocked/model"));
    assert!(requests[1].body.contains("good/model"));
}

#[tokio::test]
async fn when_every_catalog_model_is_refused_the_error_says_so() {
    let relay = entitlement_relay(true);
    let settings = LlmSettings::new(
        LlmProvider::OrcaRouter,
        "sk-orca-unit-test-key",
        "blocked/model",
        relay.bases(),
    );

    let error = settings
        .complete_text("hi".to_string(), "blocked/model".to_string())
        .await
        .expect_err("no catalog model is usable");

    assert!(
        matches!(&error, ProviderError::ModelNotEntitled { .. }),
        "expected an entitlement error, got {error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("403"), "{message}");
    assert!(message.contains("--model"), "the user is told how to recover: {message}");
    assert_eq!(chat_requests(&relay).len(), 2, "each catalog model is tried at most once");
}

// ---------------------------------------------------------------------------
// Live request through the implemented provider
// ---------------------------------------------------------------------------

/// Exercises the real relay with a real credential.
///
/// Skipped when `ORCAROUTER_API_KEY` is absent. `orcacyber-zero` is the documented default; when
/// the workspace has not unlocked the cyber route the provider reports `403` and this test falls
/// back to `orcarouter/fusion`, which is exactly the documented behaviour.
#[test]
fn live_request_through_the_orcarouter_provider() {
    let key = match std::env::var("ORCAROUTER_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            eprintln!("ORCAROUTER_API_KEY not set; skipping the live request");
            return;
        }
    };

    let bases = OrcaRouterBases::default();

    let models = discover_models(
        &bases,
        &key,
        heimdall_common::resources::orcarouter::Capability::Chat,
    )
    .expect("the live model catalog should be readable");
    assert!(!models.is_empty(), "the live catalog must advertise at least one chat model");
    assert!(
        models.iter().all(|model| model.supports_chat()),
        "every discovered chat model must pass the chat capability filter"
    );

    let preferred = models
        .iter()
        .find(|model| model.id == DEFAULT_MODEL)
        .map(|model| model.id.clone());

    // Try the documented default first, then the documented fallback, then anything the catalog
    // advertises and this key is allowed to use. A 403 means the key is not entitled to that
    // model; it is not a wiring failure, so the test keeps looking.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(model) = preferred {
        candidates.push(model);
    }
    candidates.push(FALLBACK_MODEL.to_string());
    for model in &models {
        if !candidates.contains(&model.id) {
            candidates.push(model.id.clone());
        }
    }
    candidates.truncate(6);

    let mut reply = None;
    let mut last_error = None;
    for candidate in &candidates {
        match complete_chat("Reply with the single word: ok", &key, candidate, &bases) {
            Ok(text) => {
                eprintln!(
                    "live request succeeded with '{}' ({} chat models discovered)",
                    candidate,
                    models.len()
                );
                reply = Some(text);
                break;
            }
            Err(error @ ProviderError::ModelNotEntitled { .. }) => {
                eprintln!("not entitled to '{candidate}': {error}");
                last_error = Some(error);
            }
            Err(error) => panic!("live request failed: {error}"),
        }
    }

    let reply = reply.unwrap_or_else(|| {
        panic!("no catalog model could be called with this key: {last_error:?}")
    });

    assert!(!reply.trim().is_empty(), "the relay returned an empty completion");
    assert!(!reply.contains(&key), "the response must not echo the key");
}
