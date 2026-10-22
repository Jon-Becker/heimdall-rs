//! Credential-acquisition tests for the OrcaRouter provider.
//!
//! These exercise the two adapters through the shared [`CredentialSource`] seam. Everything here
//! runs against a local fake auth origin; no real OrcaRouter account is touched and no real key is
//! used.

use heimdall_common::resources::orcarouter::{
    auth_exchange_url, authorize_url, base64url, code_challenge, generate_state, generate_verifier,
    scope_allows_inference, ApiKeyAdapter, AuthError, CredentialKind, CredentialSource,
    CredentialStore, OrcaCredential, OrcaRouterBases, PkceAdapter,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FAKE_KEY: &str = "sk-orca-test-0123456789abcdef";

/// A recorded request to the fake auth origin.
#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    body: String,
}

/// A tiny HTTP server standing in for the OrcaRouter auth origin.
struct FakeAuthServer {
    base: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
    hits: Arc<AtomicUsize>,
}

impl FakeAuthServer {
    /// Starts a server that answers `POST /api/v1/auth/keys` with `response` for the first
    /// `successes` calls and 403 afterwards.
    fn start(status: u16, response: String) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fake auth server");
        let port = listener.local_addr().expect("addr").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let hits = Arc::new(AtomicUsize::new(0));

        let requests_thread = Arc::clone(&requests);
        let hits_thread = Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let seen = hits_thread.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if seen == 0 {
                    (status, response.clone())
                } else {
                    (403, r#"{"error":"invalid_grant"}"#.to_string())
                };
                handle_request(stream, status, &body, &requests_thread);
            }
        });

        FakeAuthServer { base: format!("http://127.0.0.1:{port}"), requests, hits }
    }

    fn base(&self) -> &str {
        &self.base
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().expect("requests lock").clone()
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

fn handle_request(
    mut stream: TcpStream,
    status: u16,
    body: &str,
    requests: &Arc<Mutex<Vec<Recorded>>>,
) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end;
    loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            header_end = buffer.len();
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_subslice(&buffer, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);

    let mut body_bytes = buffer[header_end..].to_vec();
    while body_bytes.len() < content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        body_bytes.extend_from_slice(&chunk[..read]);
    }

    requests
        .lock()
        .expect("requests lock")
        .push(Recorded { method, path, body: String::from_utf8_lossy(&body_bytes).to_string() });

    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// Extracts a query parameter from an authorize URL without pulling in a URL parser.
fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if key == name {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or_default();
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Runs a full loopback login against `server`, optionally answering the callback with a
/// different `state`, a denial, or no callback at all.
fn run_login(
    server: &FakeAuthServer,
    callback: Callback,
    timeout: Duration,
) -> Result<OrcaCredential, AuthError> {
    let bases = OrcaRouterBases { auth: server.base().to_string(), api: server.base().to_string() };
    let opener: heimdall_common::resources::orcarouter::auth::BrowserOpener =
        Arc::new(move |url: &str| {
            let callback_url = query_param(url, "callback_url").expect("callback_url");
            let state = query_param(url, "state").expect("state");
            let callback = callback.clone();
            std::thread::spawn(move || {
                let target = match callback {
                    Callback::Approve { code } => {
                        format!("{callback_url}?code={code}&state={state}")
                    }
                    Callback::WrongState { code } => {
                        format!("{callback_url}?code={code}&state=not-the-state")
                    }
                    Callback::Deny => format!("{callback_url}?error=access_denied&state={state}"),
                    Callback::Silent => return,
                };
                let rest = callback_url
                    .strip_prefix("http://")
                    .expect("loopback callback is plaintext")
                    .to_string();
                let (authority, path) = match rest.split_once('/') {
                    Some((authority, path)) => (authority.to_string(), path.to_string()),
                    None => return,
                };
                let request_target = match target.split_once('?') {
                    Some((_, query)) => format!("/{path}?{query}"),
                    None => format!("/{path}"),
                };
                if let Ok(mut stream) = TcpStream::connect(&authority) {
                    let request = format!(
                        "GET {request_target} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(request.as_bytes());
                    let _ = stream.flush();
                }
            });
        });

    let adapter = PkceAdapter::new(bases, "Heimdall Test", opener, timeout);
    adapter.acquire()
}

#[derive(Clone)]
enum Callback {
    Approve { code: String },
    WrongState { code: String },
    Deny,
    Silent,
}

// ---------------------------------------------------------------------------
// API-key adapter
// ---------------------------------------------------------------------------

#[test]
fn api_key_adapter_returns_a_credential() {
    let adapter = ApiKeyAdapter::new(FAKE_KEY);
    let credential = adapter.acquire().expect("api key adapter should accept a well-formed key");
    assert_eq!(credential.key(), FAKE_KEY);
    assert_eq!(credential.source, CredentialKind::ApiKey);
    assert_eq!(credential.source.provider_id(), "orcarouter");
    assert!(credential.allows_inference());
}

#[test]
fn api_key_adapter_rejects_malformed_and_empty_keys() {
    assert_eq!(ApiKeyAdapter::new("not-a-key").acquire(), Err(AuthError::MalformedKey));
    assert_eq!(ApiKeyAdapter::new("").acquire(), Err(AuthError::NotConfigured));
    assert_eq!(ApiKeyAdapter::new("   ").acquire(), Err(AuthError::NotConfigured));
}

#[test]
fn api_key_is_masked_and_never_debug_printed() {
    let credential = ApiKeyAdapter::new(FAKE_KEY).acquire().expect("credential");
    let masked = credential.masked_key();
    assert!(masked.starts_with("sk-orca-"), "masked key keeps the provider prefix: {masked}");
    assert_ne!(masked, FAKE_KEY);
    assert!(!masked.contains("0123456789"));

    let debug = format!("{credential:?}");
    assert!(!debug.contains(FAKE_KEY), "Debug must not leak the key: {debug}");
    assert!(debug.contains("sk-orca-"));
}

#[test]
fn both_adapters_produce_the_same_credential_shape() {
    // The downstream provider only sees a key, a source tag, and a scope. Nothing about the
    // acquisition path changes how the credential is used.
    let api_key = ApiKeyAdapter::new(FAKE_KEY).acquire().expect("credential");

    let server = FakeAuthServer::start(
        200,
        format!(r#"{{"key":"{FAKE_KEY}","user_id":"12345","scope":"api"}}"#),
    );
    let pkce = run_login(
        &server,
        Callback::Approve { code: "one-time-code".to_string() },
        Duration::from_secs(5),
    )
    .expect("pkce login should succeed");

    assert_eq!(api_key.key(), pkce.key());
    assert_eq!(api_key.scope, pkce.scope);
    assert!(api_key.allows_inference() && pkce.allows_inference());
    assert_ne!(api_key.source, pkce.source, "the provenance stays distinguishable");
    assert_eq!(pkce.source.provider_id(), "orcarouter-oauth");
}

// ---------------------------------------------------------------------------
// PKCE primitives
// ---------------------------------------------------------------------------

#[test]
fn challenge_is_unpadded_s256_of_the_verifier() {
    // RFC 7636 appendix B test vector.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    assert!(!code_challenge(verifier).contains('='), "base64url must be unpadded");
}

#[test]
fn verifier_and_state_are_fresh_per_attempt() {
    let verifiers: Vec<String> = (0..8).map(|_| generate_verifier()).collect();
    let unique: std::collections::HashSet<_> = verifiers.iter().collect();
    assert_eq!(unique.len(), verifiers.len(), "each attempt needs a fresh verifier");

    let states: Vec<String> = (0..8).map(|_| generate_state()).collect();
    let unique: std::collections::HashSet<_> = states.iter().collect();
    assert_eq!(unique.len(), states.len(), "each attempt needs a fresh state");

    // 32 random bytes encode to 43 base64url characters.
    assert_eq!(verifiers[0].len(), 43);
    assert!(verifiers[0].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn base64url_matches_known_values() {
    assert_eq!(base64url(b""), "");
    assert_eq!(base64url(b"f"), "Zg");
    assert_eq!(base64url(b"fo"), "Zm8");
    assert_eq!(base64url(b"foo"), "Zm9v");
}

#[test]
fn authorize_url_targets_the_auth_origin_and_forces_s256() {
    let bases = OrcaRouterBases::default();
    let url = authorize_url(
        &bases,
        "http://127.0.0.1:51733/cb",
        "challenge-value",
        "S256",
        "state-value",
        "Heimdall",
        "api",
    );

    assert!(url.starts_with("https://www.orcarouter.ai/auth?"), "{url}");
    assert!(!url.contains("api.orcarouter.ai"), "authorize never goes to the inference origin");
    assert_eq!(query_param(&url, "code_challenge_method").as_deref(), Some("S256"));
    assert_eq!(
        query_param(&url, "callback_url").as_deref(),
        Some("http://127.0.0.1:51733/cb")
    );
    assert_eq!(query_param(&url, "app_name").as_deref(), Some("Heimdall"));
    assert_eq!(query_param(&url, "scope").as_deref(), Some("api"));
    // The verifier never rides on the URL; only its hash does.
    assert!(!url.contains("verifier"));
}

#[test]
fn exchange_url_is_the_api_v1_auth_path_on_the_auth_origin() {
    let bases = OrcaRouterBases::default();
    let url = auth_exchange_url(&bases);
    assert_eq!(url, "https://www.orcarouter.ai/api/v1/auth/keys");
    assert!(!url.contains("api.orcarouter.ai"), "the relay host does not serve auth endpoints");
    assert!(url.ends_with("/api/v1/auth/keys"), "the auth path carries the /api prefix: {url}");
    assert!(
        !url.starts_with("https://www.orcarouter.ai/v1/"),
        "the relay's /v1 prefix does not apply to auth endpoints: {url}"
    );
}

// ---------------------------------------------------------------------------
// PKCE end-to-end against the fake auth origin
// ---------------------------------------------------------------------------

#[test]
fn pkce_login_exchanges_the_code_and_returns_a_key() {
    let server = FakeAuthServer::start(
        200,
        format!(r#"{{"key":"{FAKE_KEY}","user_id":"12345","scope":"api"}}"#),
    );

    let credential = run_login(
        &server,
        Callback::Approve { code: "one-time-code".to_string() },
        Duration::from_secs(5),
    )
    .expect("login should succeed");

    assert_eq!(credential.key(), FAKE_KEY);
    assert_eq!(credential.source, CredentialKind::Pkce);
    assert_eq!(credential.account, "12345");
    assert_eq!(credential.scope, "api");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/api/v1/auth/keys");
    assert!(requests[0].body.contains("one-time-code"));
    assert!(requests[0].body.contains("\"code_challenge_method\":\"S256\""));
    assert!(requests[0].body.contains("code_verifier"));
}

#[test]
fn pkce_denial_is_reported_and_no_exchange_happens() {
    let server =
        FakeAuthServer::start(200, format!(r#"{{"key":"{FAKE_KEY}","scope":"api"}}"#));

    let error = run_login(&server, Callback::Deny, Duration::from_secs(5))
        .expect_err("a denial must not produce a credential");

    assert_eq!(error, AuthError::Denied);
    assert_eq!(server.hits(), 0, "a denied login must not call the exchange endpoint");
}

#[test]
fn pkce_state_mismatch_is_rejected_before_exchange() {
    let server =
        FakeAuthServer::start(200, format!(r#"{{"key":"{FAKE_KEY}","scope":"api"}}"#));

    let error = run_login(
        &server,
        Callback::WrongState { code: "attacker-code".to_string() },
        Duration::from_secs(5),
    )
    .expect_err("a mismatched state must be rejected");

    assert_eq!(error, AuthError::StateMismatch);
    assert_eq!(server.hits(), 0, "the code must not be redeemed after a state mismatch");
}

#[test]
fn pkce_timeout_ends_without_hanging() {
    let server =
        FakeAuthServer::start(200, format!(r#"{{"key":"{FAKE_KEY}","scope":"api"}}"#));

    let started = std::time::Instant::now();
    let error = run_login(&server, Callback::Silent, Duration::from_millis(300))
        .expect_err("a silent browser must time out");

    assert_eq!(error, AuthError::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5), "timeout must be prompt");
    assert_eq!(server.hits(), 0);
}

#[test]
fn pkce_scope_downgrade_is_refused() {
    let server = FakeAuthServer::start(
        200,
        format!(r#"{{"key":"{FAKE_KEY}","user_id":"12345","scope":"connector"}}"#),
    );

    let error = run_login(
        &server,
        Callback::Approve { code: "one-time-code".to_string() },
        Duration::from_secs(5),
    )
    .expect_err("a scope that cannot call inference must be refused");

    assert_eq!(error, AuthError::InsufficientScope { granted: "connector".to_string() });
    assert!(!scope_allows_inference("connector"));
    assert!(scope_allows_inference("api"));
}

#[test]
fn pkce_code_reuse_is_reported_as_a_terminal_failure() {
    let server =
        FakeAuthServer::start(200, format!(r#"{{"key":"{FAKE_KEY}","scope":"api"}}"#));

    // The first login consumes the one-time code; the fake origin answers 403 afterwards.
    run_login(&server, Callback::Approve { code: "one-time-code".to_string() }, Duration::from_secs(5))
        .expect("first login");

    let error = run_login(
        &server,
        Callback::Approve { code: "one-time-code".to_string() },
        Duration::from_secs(5),
    )
    .expect_err("a reused code must fail");

    assert!(
        matches!(error, AuthError::ExchangeRejected { status: 403, .. }),
        "expected a 403 exchange rejection, got {error:?}"
    );
    assert!(!error.to_string().contains(FAKE_KEY), "the error must not echo a key");
}

// ---------------------------------------------------------------------------
// Credential generation safety
// ---------------------------------------------------------------------------

#[test]
fn a_stale_attempt_cannot_overwrite_a_newer_credential() {
    let store = CredentialStore::new();
    let first = store.begin_attempt();
    let second = store.begin_attempt();

    let mut late = OrcaCredential::new("sk-orca-late-key-0001", CredentialKind::Pkce);
    late.account = "old".to_string();

    assert!(store.commit(first, late).is_none(), "a late result from an old attempt is dropped");

    let fresh = OrcaCredential::new("sk-orca-new-key-0002", CredentialKind::Pkce);
    let committed = store.commit(second, fresh).expect("the current attempt may commit");
    assert_eq!(committed.generation, second);
    assert_eq!(committed.key(), "sk-orca-new-key-0002");
}

#[test]
fn a_revoked_credential_is_a_terminal_reauth_signal_not_a_refresh() {
    // A PKCE-issued key is a durable grant, not a refresh token: there is no refresh path to call,
    // so a 401 has to surface as "re-authorize", never as a retry loop.
    let credential = ApiKeyAdapter::new(FAKE_KEY).acquire().expect("credential");
    assert!(credential.allows_inference());

    let error = heimdall_common::resources::orcarouter::classify_status(401, "orcacyber-zero", "");
    assert_eq!(
        error,
        Some(heimdall_common::resources::orcarouter::ProviderError::Unauthorized)
    );
    let message = error.expect("error").to_string();
    assert!(message.contains("orcarouter login"), "the user is told how to recover: {message}");
}
