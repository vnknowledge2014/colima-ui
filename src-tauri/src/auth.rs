//! Authentication infrastructure for the HTTP API server.
//!
//! Generates a CSPRNG-based auth token and provides middleware for Axum.
//!
//! # How the token reaches a client
//!
//! There is deliberately **no unauthenticated HTTP endpoint** that hands the
//! token out. There used to be (`GET /api/auth/token`, in the public router),
//! guarded only by CORS — which restricts browsers and nothing else. Any local
//! process could read the token and then use the whole API: writing arbitrary
//! files to the host, applying compose patches, driving container lifecycle.
//!
//! Clients get it without that endpoint:
//!
//! - **Desktop (Tauri):** [`api_token`] over IPC. The webview is the app; it is
//!   already inside the trust boundary. Unrelated local processes are not.
//! - **Browser:** out-of-band, in a URL fragment (`#token=…`). Fragments are
//!   never sent to a server, so the token stays out of request logs and out of
//!   `Referer`. The frontend reads it once and strips it from the address bar.
//!
//! On the browser side, "out-of-band" today means exactly one thing: a debug
//! build prints the ready-made URL to stdout when the server binds. There is no
//! in-app "open in browser" affordance, because there is no browser mode to
//! open one for — this router serves no frontend (no `ServeDir` anywhere), so
//! the only page that can reach it is the vite dev server on 1420. Browser mode
//! is a development surface, and the handoff matches that.
//!
//! If a packaged build ever serves its own frontend, this is the seam that has
//! to grow a real handoff; until then, promising one would be fiction.
//!
//! Either way a tab opened by hand has no credential and no way to obtain one.
//! That is the point, and it is the fix `docs/architecture.md` already named.

use axum::{
    http::{header, StatusCode},
    middleware::Next,
};
use std::sync::OnceLock;

static API_TOKEN: OnceLock<String> = OnceLock::new();

/// Endpoints that authenticate via `?token=` instead of the `Authorization`
/// header, because the browser `EventSource` API cannot set headers.
///
/// This is an explicit list, not a path pattern. The previous
/// `path.ends_with("/stream")` rule meant any future route whose name happened
/// to end in `/stream` would silently switch authentication mechanism — and the
/// P0 plan adds streaming surface.
const QUERY_TOKEN_PATHS: &[&str] = &["/api/events", "/api/k8s/pods/logs/stream"];

/// Get the API token, generating a cryptographically random one on first call.
/// Using a CSPRNG (instead of the previous timestamp+pid scheme) prevents an
/// attacker from guessing/brute-forcing the token within a short time window.
pub fn get_api_token() -> String {
    API_TOKEN
        .get_or_init(|| {
            use rand::RngCore;
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
            format!("colima-{}", hex)
        })
        .clone()
}

/// Hand the API token to the app's own webview, over IPC.
///
/// IPC is only reachable from the webview this app owns, so unlike the HTTP
/// endpoint this replaces, another local process cannot call it.
///
/// The desktop app routes almost everything through Tauri commands and needs no
/// token at all; the exception is SSE, because the browser `EventSource` API
/// cannot set an `Authorization` header and must pass `?token=` instead.
#[tauri::command]
pub fn api_token() -> String {
    get_api_token()
}

/// Compare two secrets without an early return on the first differing byte.
///
/// The length check is not secret: the token length is fixed and public.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extract the value of the `token` query parameter.
///
/// Parses the query properly rather than substring-matching the whole string,
/// so `?redirect=token%3Dsomething` cannot be mistaken for a credential.
fn token_from_query(query: &str) -> Option<&str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == "token")
        .map(|(_, v)| v)
}

/// Extract the credential from an `Authorization: Bearer <token>` header.
fn token_from_header(value: &str) -> Option<&str> {
    let (scheme, credential) = value.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("Bearer") {
        Some(credential.trim())
    } else {
        None
    }
}

/// Decide whether a request is authorized. Split out from the middleware so it
/// can be unit tested without constructing an Axum request.
fn is_authorized(
    path: &str,
    query: Option<&str>,
    auth_header: Option<&str>,
    expected: &str,
) -> bool {
    if QUERY_TOKEN_PATHS.contains(&path) {
        return query
            .and_then(token_from_query)
            .is_some_and(|t| constant_time_eq(t, expected));
    }

    auth_header
        .and_then(token_from_header)
        .is_some_and(|t| constant_time_eq(t, expected))
}

/// Auth middleware — rejects requests without a valid Bearer token.
/// SSE endpoints listed in [`QUERY_TOKEN_PATHS`] use `?token=` instead.
pub async fn auth_middleware(
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected = get_api_token();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| q.to_string());
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if is_authorized(&path, query.as_deref(), auth_header.as_deref(), &expected) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "colima-0123456789abcdef";

    #[test]
    fn accepts_correct_bearer_header() {
        let header = format!("Bearer {}", TOKEN);
        assert!(is_authorized("/api/containers", None, Some(&header), TOKEN));
    }

    #[test]
    fn rejects_missing_or_malformed_header() {
        assert!(!is_authorized("/api/containers", None, None, TOKEN));
        assert!(!is_authorized("/api/containers", None, Some(TOKEN), TOKEN));
        assert!(!is_authorized("/api/containers", None, Some("Basic dXNlcjpwdw=="), TOKEN));
        assert!(!is_authorized("/api/containers", None, Some("Bearer"), TOKEN));
    }

    #[test]
    fn rejects_prefix_and_suffix_of_the_token() {
        let prefix = format!("Bearer {}", &TOKEN[..TOKEN.len() - 1]);
        let suffix = format!("Bearer {}x", TOKEN);
        assert!(!is_authorized("/api/containers", None, Some(&prefix), TOKEN));
        assert!(!is_authorized("/api/containers", None, Some(&suffix), TOKEN));
    }

    #[test]
    fn sse_paths_accept_a_parsed_query_token() {
        let q = format!("token={}", TOKEN);
        assert!(is_authorized("/api/events", Some(&q), None, TOKEN));

        let with_others = format!("ns=default&token={}&follow=true", TOKEN);
        assert!(is_authorized("/api/k8s/pods/logs/stream", Some(&with_others), None, TOKEN));
    }

    #[test]
    fn sse_paths_reject_a_token_that_is_merely_embedded_in_the_query() {
        // The old implementation substring-matched the raw query string, so a
        // value that merely contained "token=<secret>" would pass.
        let sneaky = format!("redirect=http://evil.test/?token={}", TOKEN);
        assert!(!is_authorized("/api/events", Some(&sneaky), None, TOKEN));
    }

    #[test]
    fn query_token_does_not_work_on_regular_paths() {
        let q = format!("token={}", TOKEN);
        assert!(!is_authorized("/api/containers", Some(&q), None, TOKEN));
    }

    #[test]
    fn header_does_not_work_on_query_token_paths() {
        // Keeps the two mechanisms from silently overlapping.
        let header = format!("Bearer {}", TOKEN);
        assert!(!is_authorized("/api/events", None, Some(&header), TOKEN));
    }

    #[test]
    fn unlisted_stream_suffix_uses_header_auth() {
        // Regression guard: a new route ending in "/stream" must not switch to
        // query-token auth just because of its name.
        let q = format!("token={}", TOKEN);
        assert!(!is_authorized("/api/future/stream", Some(&q), None, TOKEN));

        let header = format!("Bearer {}", TOKEN);
        assert!(is_authorized("/api/future/stream", None, Some(&header), TOKEN));
    }

    #[test]
    fn constant_time_eq_matches_semantics_of_equality() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }
}
