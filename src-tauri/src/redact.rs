//! Redaction of secrets from any string that may reach the user, a log, or a
//! bug report.
//!
//! Error strings are the main leak path: `reqwest` includes the full request
//! URL in its `Display` output, so an error from a provider that takes its key
//! as a query parameter carries the key with it. That string then reaches a
//! toast the user copies into a GitHub issue.
//!
//! Two rules keep this robust as providers come and go:
//!   1. Redact by *position* — anything in a credential-shaped query parameter
//!      or auth header is redacted regardless of what the value looks like.
//!   2. Redact by *shape* — known key formats are caught even when they appear
//!      somewhere unexpected (a JSON body echoed back, a shell command).
//!
//! Rule 1 is what protects providers we have never heard of.

use regex_lite::Regex;
use std::sync::LazyLock;

/// What replaces a redacted value. Keeps the parameter/header name visible so
/// the message stays useful for debugging.
const MASK: &str = "<redacted>";

/// Credential-bearing query parameters: `?key=`, `&api_key=`, `?access_token=` …
static QUERY_PARAM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([?&](?:api[_-]?key|apikey|access[_-]?token|auth[_-]?token|key|token|auth|secret|password)=)[^&\s\)\]}\x22']+")
        .expect("QUERY_PARAM regex is valid")
});

/// `Authorization: Bearer <token>` in any casing, including inside a formatted
/// header dump.
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._\-]{8,}").expect("BEARER regex is valid"));

/// Header-style credentials: `x-api-key: sk-…`, `"x-goog-api-key": "AIza…"`.
///
/// The value class deliberately allows one optional space-separated second
/// token, so `Authorization: Basic dXNlcjpwdw==` masks the credential and not
/// just the word `Basic`. [`BEARER`] handles the bearer scheme on its own.
static HEADER_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)((?:x-api-key|x-goog-api-key|api-key|authorization)\x22?\s*[:=]\s*\x22?)[^\s,;\x22'}\)]+(\s+[^\s,;\x22'}\)]+)?"#,
    )
    .expect("HEADER_KEY regex is valid")
});

/// Provider key shapes. Deliberately conservative: patterns must be distinctive
/// enough that they cannot match legitimate output.
///
/// Notably absent are bare hex/alphanumeric runs (Together, Mistral). A 64-char
/// hex pattern would also match Docker image digests, which we must keep
/// readable — a false positive there would break diagnostics for every user to
/// protect a minority of providers. Rule 1 (position) covers those keys instead.
static KEY_SHAPES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // OpenAI, Anthropic (sk-ant-), OpenRouter (sk-or-), DeepSeek, Together
        r"sk-[A-Za-z0-9._\-]{16,}",
        // Google AI Studio / Gemini
        r"AIza[A-Za-z0-9_\-]{20,}",
        // Groq
        r"gsk_[A-Za-z0-9]{20,}",
        // Hugging Face
        r"hf_[A-Za-z0-9]{16,}",
        // GitHub tokens, in case one ends up in a diagnostic bundle
        r"gh[pousr]_[A-Za-z0-9]{16,}",
        // This app's own HTTP API bearer token (see auth::get_api_token)
        r"colima-[0-9a-f]{64}",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("key shape regex is valid"))
    .collect()
});

/// Strip credentials from a string before it is shown, logged, or reported.
///
/// Always apply this at the point the string is *built*, not at the point it is
/// displayed — a string that is never redacted at the source will eventually
/// escape through a path nobody remembered to cover.
pub fn redact(input: &str) -> String {
    let mut out = QUERY_PARAM
        .replace_all(input, format!("${{1}}{}", MASK).as_str())
        .into_owned();
    out = BEARER
        .replace_all(&out, format!("${{1}}{}", MASK).as_str())
        .into_owned();
    out = HEADER_KEY
        .replace_all(&out, format!("${{1}}{}", MASK).as_str())
        .into_owned();

    for re in KEY_SHAPES.iter() {
        out = re.replace_all(&out, MASK).into_owned();
    }
    out
}

/// Convenience wrapper for the common `map_err(|e| format!("...: {}", e))`
/// pattern, so callers cannot forget the redaction step.
pub fn redact_err<E: std::fmt::Display>(context: &str, e: E) -> String {
    redact(&format!("{}: {}", context, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_gemini_key_in_query_string() {
        let err = "Request failed: error sending request for url \
                   (https://generativelanguage.googleapis.com/v1beta/models?key=AIzaSyC1234567890abcdefghijklmnop)";
        let out = redact(err);
        assert!(!out.contains("AIzaSyC1234567890abcdefghijklmnop"), "key leaked: {out}");
        assert!(out.contains("key=<redacted>"), "parameter name should survive: {out}");
    }

    #[test]
    fn redacts_unknown_provider_key_by_position() {
        // A provider we have never seen, with a key shape we do not recognise.
        let out = redact("GET https://example.invalid/v1/models?api_key=zzzz-not-a-known-shape-9999 failed");
        assert!(!out.contains("zzzz-not-a-known-shape-9999"), "key leaked: {out}");
    }

    #[test]
    fn redacts_bearer_token() {
        let out = redact("headers: {authorization: Bearer sk-proj-abcdefghijklmnopqrstuvwxyz}");
        assert!(!out.contains("sk-proj-abcdefghijklmnopqrstuvwxyz"), "token leaked: {out}");
    }

    #[test]
    fn redacts_header_style_api_key() {
        let out = redact(r#"{"x-api-key": "sk-ant-api03-AAAABBBBCCCCDDDDEEEE"}"#);
        assert!(!out.contains("sk-ant-api03-AAAABBBBCCCCDDDDEEEE"), "key leaked: {out}");
    }

    #[test]
    fn redacts_known_key_shapes_anywhere() {
        for key in [
            "sk-abcdefghijklmnopqrstuvwxyz123456",
            "AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz01234",
            "gsk_abcdefghijklmnopqrstuvwxyz0123",
            "hf_abcdefghijklmnopqrst",
            "ghp_abcdefghijklmnopqrst",
        ] {
            let out = redact(&format!("something went wrong near {key} while running"));
            assert!(!out.contains(key), "shape not redacted: {key} -> {out}");
        }
    }

    #[test]
    fn redacts_non_bearer_authorization_schemes() {
        let out = redact("Authorization: Basic dXNlcjpwYXNzd29yZA==");
        assert!(!out.contains("dXNlcjpwYXNzd29yZA=="), "basic credential leaked: {out}");
    }

    #[test]
    fn redacts_the_apps_own_api_token() {
        let token = format!("colima-{}", "a1b2c3d4".repeat(8));
        let out = redact(&format!("GET /api/events?token={token} returned 401"));
        assert!(!out.contains(&token), "api token leaked: {out}");
    }

    #[test]
    fn preserves_docker_image_digests() {
        // A 64-char hex digest must stay readable — redacting it would break
        // diagnostics for everyone to protect a couple of providers.
        let digest = "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        let out = redact(&format!("image {digest} not found"));
        assert!(out.contains(digest), "digest was mangled: {out}");
    }

    #[test]
    fn leaves_ordinary_errors_untouched() {
        let msg = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock";
        assert_eq!(redact(msg), msg);
    }

    #[test]
    fn redact_err_applies_redaction() {
        let out = redact_err("Request failed", "url https://x.test/v1?key=AIzaSy0123456789abcdefghijkl");
        assert!(!out.contains("AIzaSy0123456789abcdefghijkl"), "key leaked: {out}");
        assert!(out.starts_with("Request failed:"));
    }
}
