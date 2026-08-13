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
//!
//! # Cost
//!
//! `regex_lite` is automata-based, so there is no backtracking blowup — cost is
//! linear in input length, times the ~13 passes below. Measured at roughly
//! 0.8 s per megabyte in a release build, and around ten times that in a debug
//! build.
//!
//! That is fine for the error strings this was built for, and fine for a
//! diagnostic bundle **redacted section by section**, which is how the bundle is
//! assembled. Handing this one multi-megabyte string on the UI thread would be
//! visible as a hang; if that ever becomes the shape of a caller, move the call
//! off-thread rather than weakening the rules.

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

/// `KEY=VALUE` where the key names a credential: `POSTGRES_PASSWORD=…`,
/// `"AWS_SECRET_ACCESS_KEY": "…"`, `--token=…`.
///
/// This is rule 1 (position) applied to the second leak path. The module was
/// written for error strings, where a secret is always in a URL or a header;
/// diagnostic bundles add container logs and `docker inspect`, where secrets are
/// plain assignments instead.
///
/// Deliberately **not** line-anchored: `docker inspect` emits env as a JSON
/// array, so the assignment sits mid-line between quotes.
///
/// Credential-bearing key names. Closed list, like [`QUERY_PARAM`] — matching
/// any assignment whose value merely looks random would swallow image tags,
/// container ids and digests.
///
/// `_KEY` carries the leading underscore on purpose: it catches
/// `SUPABASE_SERVICE_ROLE_KEY` and `SUPABASE_ANON_KEY` without also matching
/// `MONKEY`.
const SECRET_WORDS: &str = concat!(
    "PASSWORD|PASSWD|PASSPHRASE|REQUIREPASS|SECRET|TOKEN",
    "|APIKEY|API[_-]?KEY|_KEY|PRIVATE[_-]?KEY|ACCESS[_-]?KEY",
    "|CREDENTIALS?|SALT|DSN"
);

/// What a secret's value may contain.
///
/// Three exclusions earn their place:
/// - `<` and `>` keep the rule off [`MASK`] itself. Without them a second pass
///   matched `token=<redacted>&token=def` and deleted everything after the mask,
///   including other parameters that still needed redacting.
/// - `&` stops a query-string match from running past one parameter into the
///   next, so `?api_key=…&model=gpt-4` keeps its `model`.
/// - The `{1,512}` bound stops one `PASSWORD=` in a megabyte of token-free log
///   from collapsing the rest of the bundle into 19 bytes.
const SECRET_VALUE: &str = r#"[^\s,;\x22'<>&}\)\]]{1,512}"#;

/// `KEY=VALUE`. Case-insensitive: `--password=…` on a command line is as real as
/// `POSTGRES_PASSWORD=…` in an environment.
///
/// Spacing is `[ \t]`, never `\s`: an assignment is a same-line construct, and
/// `\s` spans newlines, so `secret:` at the end of one line would consume the
/// first token of the next.
static ASSIGN_EQ: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)([A-Za-z0-9_]*(?:{SECRET_WORDS})[A-Za-z0-9_]*\x22?[ \t]*=[ \t]*\x22?){SECRET_VALUE}"#
    ))
    .expect("ASSIGN_EQ regex is valid")
});

/// `--flag VALUE` — a command-line secret separated by a space rather than `=`,
/// as `redis-server --requirepass …` and `mysql --password …` are written.
///
/// The leading dash is what makes a space separator safe here: without it the
/// same rule would match any sentence containing the word "token".
static ASSIGN_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)(--?[A-Za-z0-9_-]*(?:{SECRET_WORDS})[A-Za-z0-9_-]*[ \t]+){SECRET_VALUE}"#
    ))
    .expect("ASSIGN_FLAG regex is valid")
});

/// `"KEY": "VALUE"` — a quoted key, as `docker inspect` and any JSON config
/// emit. The quotes are what make this unambiguous, so the key may be any case.
static ASSIGN_JSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)(\x22[A-Za-z0-9_]*(?:{SECRET_WORDS})[A-Za-z0-9_]*\x22[ \t]*:[ \t]*\x22?){SECRET_VALUE}"#
    ))
    .expect("ASSIGN_JSON regex is valid")
});

/// `KEY: VALUE` with a bare key — YAML, and log lines shaped like it.
///
/// **Case-sensitive, and the key must be `UPPER_SNAKE`.** A colon after an
/// ordinary English word is how error messages are written, and a case-blind
/// rule ate the diagnosis out of real Docker output:
///
/// ```text
/// Error saving credentials: error storing credentials - err: exit status 1
/// services.db.secrets: must be a list
/// ```
///
/// Requiring `SCREAMING_SNAKE` keeps the rule on things that are actually
/// variable names. [`ASSIGN_YAML_LOWER`] handles the narrow set of lowercase
/// keys that are worth the risk.
static ASSIGN_YAML_UPPER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"([A-Z][A-Z0-9_]*(?:{SECRET_WORDS})[A-Z0-9_]*[ \t]*:[ \t]*\x22?){SECRET_VALUE}"#
    ))
    .expect("ASSIGN_YAML_UPPER regex is valid")
});

/// The lowercase `key: value` forms worth catching despite the prose risk.
///
/// Restricted to words that essentially never appear before a colon in an error
/// message, unlike `token:`, `secret:` or `credentials:`. A compose file with
/// `password: hunter2` is a real case the diagnose feature reads.
static ASSIGN_YAML_LOWER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)((?:^|[\s\-])(?:password|passwd|passphrase)[ \t]*:[ \t]*\x22?){SECRET_VALUE}"#
    ))
    .expect("ASSIGN_YAML_LOWER regex is valid")
});

/// Credentials in a URL authority: `postgres://user:pw@host/db`.
///
/// Everything between `://` and `@` is masked, and the userinfo is `*` not `+`
/// so the two credential-only forms are covered as well: `redis://:pw@host`
/// (empty username, the standard Redis form) and `https://key@host/1` (a bare
/// token, as Sentry DSNs are written).
///
/// Only the credential goes. The scheme and host are the useful half of a
/// connection error — "cannot reach db.internal:5432" is the diagnosis.
static URL_CREDENTIAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([a-z][a-z0-9+.\-]*://)[^/\s@]*@").expect("URL_CREDENTIAL regex is valid")
});

/// A PEM private key block, applied before every other rule.
///
/// Order is load-bearing. A PEM block contains spaces (`BEGIN RSA PRIVATE KEY`),
/// so any assignment rule that ran first stopped its value at that space, cut
/// the `-----BEGIN` header off, and left this pattern unable to match — the
/// whole key body then shipped in the clear. `docker inspect` produces exactly
/// that shape, on one line, with `\n` escaped.
///
/// Running first is safe because the block is self-delimiting: it cannot be
/// half-consumed by anything else if nothing else has run yet.
static PEM_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----")
        .expect("PEM_BLOCK regex is valid")
});

/// The user's account name in a home path, on the host and inside the VM.
///
/// Not a credential, so not in [`KEY_SHAPES`] — but a diagnostic bundle is
/// written to be pasted into a public issue, and the account name is personal
/// data nobody needs in order to read a stack trace. `/home/` is covered too:
/// this app drives a Linux VM, so guest-side paths appear in `colima ssh` and
/// container logs.
///
/// Applied last, so it cannot cut into a value another rule already masked.
static HOME_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/(Users|home)/([^/\s:,;\x22'\)\]]+)").expect("HOME_PATH regex is valid")
});

/// Entries under `/Users` that are not accounts. Masking these would be a
/// misleading edit rather than a privacy win.
const NON_ACCOUNT_HOME_DIRS: &[&str] = &["Shared", "Guest"];

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
        // AWS access key id. The 40-char secret key that accompanies it has no
        // distinctive prefix and is deliberately absent — matching bare base64
        // runs is exactly the false-positive class this list refuses. In
        // practice the secret arrives as `AWS_SECRET_ACCESS_KEY=…`, which
        // ENV_ASSIGNMENT covers by position.
        r"AKIA[0-9A-Z]{16}",
        // JWT: three base64url segments. Long minimums keep it from matching
        // ordinary dotted identifiers.
        r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        // PEM blocks are NOT here — see PEM_BLOCK, which must run before the
        // assignment rules rather than after them.
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
/// Order matters, and two constraints fix it:
///
/// - [`PEM_BLOCK`] runs **first**. It is the only rule whose match contains
///   spaces, so any assignment rule running earlier truncates it and prevents it
///   from matching at all.
/// - [`HOME_PATH`] runs **last**. It is identity rather than credentials, and
///   running it earlier could cut into a value another rule was still matching.
///
/// Everything between is order-independent, because [`SECRET_VALUE`] excludes
/// the characters [`MASK`] is made of — a rule cannot re-match another rule's
/// output and extend past it.
pub fn redact(input: &str) -> String {
    let keep_group_1 = format!("${{1}}{}", MASK);

    let mut out = PEM_BLOCK.replace_all(input, MASK).into_owned();

    for re in [
        &*QUERY_PARAM,
        &*BEARER,
        &*HEADER_KEY,
        &*ASSIGN_EQ,
        &*ASSIGN_FLAG,
        &*ASSIGN_JSON,
        &*ASSIGN_YAML_UPPER,
        &*ASSIGN_YAML_LOWER,
    ] {
        out = re.replace_all(&out, keep_group_1.as_str()).into_owned();
    }

    out = URL_CREDENTIAL
        .replace_all(&out, format!("${{1}}{}@", MASK).as_str())
        .into_owned();

    for re in KEY_SHAPES.iter() {
        out = re.replace_all(&out, MASK).into_owned();
    }

    HOME_PATH
        .replace_all(&out, |caps: &regex_lite::Captures| {
            let root = &caps[1];
            let name = &caps[2];
            if NON_ACCOUNT_HOME_DIRS.contains(&name) {
                format!("/{root}/{name}")
            } else {
                format!("/{root}/<user>")
            }
        })
        .into_owned()
}

/// Convenience wrapper for the common `map_err(|e| format!("...: {}", e))`
/// pattern, so callers cannot forget the redaction step.
pub fn redact_err<E: std::fmt::Display>(context: &str, e: E) -> String {
    redact(&format!("{}: {}", context, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Container-log secrets (diagnostic bundles) =====
    //
    // The module was built for one leak path: a provider key inside an error
    // string, which is always a URL or a header. Diagnostic bundles add a second
    // one — container logs and `docker inspect` — where secrets appear as plain
    // `KEY=VALUE`. Those are the cases below.

    #[test]
    fn redacts_env_assignment_in_a_container_log() {
        let out = redact("2026-08-12 db.1  | POSTGRES_PASSWORD=hunter2 accepted");
        assert!(!out.contains("hunter2"), "password leaked: {out}");
        assert!(out.contains("POSTGRES_PASSWORD"), "key name should survive: {out}");
    }

    #[test]
    fn redacts_env_assignment_inside_docker_inspect_json() {
        // `docker inspect` emits env as a JSON array, so the assignment is not
        // at the start of a line — the pattern must not be line-anchored.
        let out = redact(r#""Env": ["PATH=/usr/bin", "MYSQL_ROOT_PASSWORD=s3cr3t", "TZ=UTC"]"#);
        assert!(!out.contains("s3cr3t"), "password leaked: {out}");
        assert!(out.contains("PATH=/usr/bin"), "harmless env should survive: {out}");
        assert!(out.contains("TZ=UTC"), "harmless env should survive: {out}");
    }

    #[test]
    fn redacts_credentials_embedded_in_a_url() {
        let out = redact("could not connect to postgres://admin:tr0ub4dor@db.internal:5432/app");
        assert!(!out.contains("tr0ub4dor"), "password leaked: {out}");
        assert!(out.contains("db.internal:5432"), "host must stay readable: {out}");
    }

    #[test]
    fn redacts_aws_access_key_id() {
        let out = redact("using credentials AKIAIOSFODNN7EXAMPLE for s3 upload");
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"), "key id leaked: {out}");
    }

    #[test]
    fn redacts_a_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let out = redact(&format!("auth header carried {jwt} which expired"));
        assert!(!out.contains(jwt), "jwt leaked: {out}");
    }

    #[test]
    fn redacts_a_pem_private_key_block() {
        let pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA1234\nabcdEFGH\n-----END RSA PRIVATE KEY-----";
        let out = redact(&format!("mounted secret:\n{pem}\ndone"));
        assert!(!out.contains("MIIEowIBAAKCAQEA1234"), "key body leaked: {out}");
        assert!(out.contains("done"), "surrounding text should survive: {out}");
    }

    #[test]
    fn anonymises_the_home_directory() {
        // Not a secret, but a bundle is destined for a public GitHub issue and
        // the account name is personal data.
        let out = redact("failed to read /Users/longnd/.colima/default/docker.sock");
        assert!(!out.contains("longnd"), "username leaked: {out}");
        assert!(out.contains("/.colima/default/docker.sock"), "path shape should survive: {out}");
    }

    // ===== Negative cases: over-redaction breaks diagnostics =====

    /// An assignment must not reach across a line break for its value.
    ///
    /// Found the hard way: `\s*` after the separator let `secret:` at the end of
    /// one line swallow `-----BEGIN` on the next, which stopped the PEM rule
    /// from matching and left the key body unredacted. Over-redaction here
    /// caused under-redaction one rule later.
    #[test]
    fn an_assignment_does_not_consume_the_next_line() {
        let out = redact("secret:\ndocker version 27.3.1");
        assert!(out.contains("docker version 27.3.1"), "next line was eaten: {out}");
    }

    // ===== Regressions found by review, each with the input that broke it =====

    /// A PEM on the same line as the assignment — the shape `docker inspect`
    /// emits, since it escapes newlines. The first fix only covered the variant
    /// with a real line break; this one still leaked the whole key body because
    /// the assignment rule truncated `-----BEGIN` at the space inside `BEGIN RSA`.
    #[test]
    fn redacts_a_pem_assigned_on_one_line() {
        let out = redact(
            r#""Env": ["SSH_PRIVATE_KEY=-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----"]"#,
        );
        assert!(!out.contains("b3BlbnNzaC1rZXktdjEAAAAA"), "key body leaked: {out}");
    }

    /// The mask is not itself redactable. `<redacted>` contains `<` and `>`; when
    /// the value class allowed them, a later rule matched the mask and deleted
    /// everything after it — including parameters that still needed masking.
    #[test]
    fn a_mask_is_not_re_matched_and_does_not_eat_what_follows() {
        let out = redact("url https://x.test/v1?api_key=AIza0123456789abcdefghijkl&model=gpt-4 failed");
        assert!(!out.contains("AIza0123456789abcdefghijkl"), "key leaked: {out}");
        assert!(out.contains("model=gpt-4"), "following parameter was eaten: {out}");
    }

    /// Redis writes its URL with an empty username; a `+` quantifier on the
    /// userinfo missed it entirely.
    #[test]
    fn redacts_url_credentials_without_a_username() {
        let out = redact("redis://:sup3rs3cret@redis:6379 refused");
        assert!(!out.contains("sup3rs3cret"), "password leaked: {out}");
        assert!(out.contains("redis:6379"), "host must stay readable: {out}");
    }

    /// A hostname containing "token" is not an assignment. The case-blind colon
    /// rule matched `tokenuser:` and deleted the rest of the URL.
    #[test]
    fn a_keyword_inside_a_hostname_does_not_eat_the_url() {
        let out = redact("fatal: https://tokenuser:ghp_abcdefghijklmnopqrst@github.com/o/r.git");
        assert!(!out.contains("ghp_abcdefghijklmnopqrst"), "token leaked: {out}");
        assert!(out.contains("github.com/o/r.git"), "repo url was eaten: {out}");
    }

    /// Real Docker and compose errors put these words before a colon. Redacting
    /// there deletes the diagnosis and leaves the user with nothing to act on.
    #[test]
    fn keeps_prose_error_messages_intact() {
        for msg in [
            "Error saving credentials: error storing credentials - err: exit status 1",
            "services.db.secrets: must be a list",
            "error: You must be logged in (Unauthorized): token: expired at 2026-01-01",
        ] {
            assert_eq!(redact(msg), msg, "diagnosis was redacted");
        }
    }

    #[test]
    fn redacts_the_secret_env_vars_the_supported_stacks_actually_use() {
        for (input, secret) in [
            ("SUPABASE_SERVICE_ROLE_KEY=abc123notajwt", "abc123notajwt"),
            ("SUPABASE_ANON_KEY=xyz789", "xyz789"),
            ("SSL_PASSPHRASE=hunter2", "hunter2"),
            ("redis-server --requirepass hunter7", "hunter7"),
            ("GPG_PASSPHRASE: hunter3", "hunter3"),
            ("  password: hunter4", "hunter4"),
        ] {
            let out = redact(input);
            assert!(!out.contains(secret), "secret leaked from {input:?}: {out}");
        }
    }

    #[test]
    fn anonymises_a_home_path_inside_the_vm() {
        let out = redact("/home/longnd/.docker/config.json missing");
        assert!(!out.contains("longnd"), "username leaked: {out}");
    }

    #[test]
    fn keeps_shared_directories_named_correctly() {
        // `/Users/Shared` is not an account; masking it would be a misleading
        // edit rather than a privacy win.
        let msg = "reading /Users/Shared/config.yaml";
        assert_eq!(redact(msg), msg);
    }

    #[test]
    fn a_home_path_before_a_comma_keeps_the_comma() {
        let out = redact("searched /Users/longnd, /etc");
        assert!(out.contains(", /etc"), "comma and next path were eaten: {out}");
    }

    /// One `PASSWORD=` must not collapse the rest of a bundle. The value class
    /// is bounded for exactly this reason.
    #[test]
    fn a_secret_value_cannot_swallow_the_whole_bundle() {
        let tail = "z".repeat(4096);
        let out = redact(&format!("PASSWORD={tail} and then docker version 27.3.1"));
        assert!(out.contains("docker version 27.3.1"), "rest of bundle was eaten: {out}");
    }

    #[test]
    fn keeps_ordinary_env_vars_readable() {
        let msg = "NODE_ENV=production PORT=8080 COMPOSE_PROJECT_NAME=myapp";
        assert_eq!(redact(msg), msg);
    }

    #[test]
    fn keeps_image_references_and_ports_readable() {
        let msg = "container web-1 (postgres:16-alpine) listening on 127.0.0.1:5432";
        assert_eq!(redact(msg), msg);
    }

    #[test]
    fn keeps_a_url_without_credentials_readable() {
        let msg = "probing http://127.0.0.1:11420/api/health";
        assert_eq!(redact(msg), msg);
    }

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
