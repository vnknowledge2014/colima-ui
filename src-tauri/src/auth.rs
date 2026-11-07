//! Authentication infrastructure for the HTTP API server.
//!
//! Generates a CSPRNG-based auth token and provides middleware for Axum.
//!
//! # How the token reaches a client
//!
//! There is deliberately **no HTTP endpoint** (`GET /api/auth/token` or
//! otherwise) that hands the token out. An unauthenticated localhost HTTP route
//! guarded only by CORS restricts browsers and nothing else: any process on the
//! machine, a DNS-rebinding page, or a container sharing host networking could
//! fetch the bearer token and gain full control over host file transfers,
//! compose patches, and VM/container lifecycle.
//!
//! Legitimate clients receive credentials through three strictly scoped
//! out-of-band channels:
//!
//! 1. **Desktop (Tauri):** [`api_token`] over Tauri IPC. The webview is part of
//!    the application process trust boundary; arbitrary local processes cannot
//!    invoke Tauri IPC commands.
//! 2. **Local CLI (`cui`) & Host Agents:** `~/.colima-ui/api_token` (paired
//!    with `~/.colima-ui/api_port`), written when the HTTP server binds inside
//!    a `0700` directory with `0600` file permissions, `O_NOFOLLOW` symlink
//!    defense, and atomic temp-file rename, then unlinked on shutdown after
//!    constant-time verification that this process still owns the recorded
//!    token. Unlike an HTTP endpoint, filesystem access is enforced by OS
//!    kernel UID permissions: other local users, browser sandboxes, DNS
//!    rebinding attacks, and containers without `~/.colima-ui` mounted cannot
//!    read it.
//! 3. **Dev Browser (`tauri dev`):** `#token=…` URL fragment printed to stdout
//!    in debug builds only. URL fragments are never sent in HTTP requests or
//!    `Referer` headers, and the frontend strips the fragment immediately after
//!    reading it.

use axum::{
    http::{header, StatusCode},
    middleware::Next,
};
use std::io;
use std::path::Path;
use std::sync::OnceLock;

/// Filename under `~/.colima-ui` holding the active session's bearer token.
pub const TOKEN_FILE_NAME: &str = "api_token";

/// Filename under `~/.colima-ui` holding the TCP port the active HTTP server bound.
pub const PORT_FILE_NAME: &str = "api_port";

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
            let mut bytes = [0u8; 32];
            rand::fill(&mut bytes);
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
    if auth_header
        .and_then(token_from_header)
        .is_some_and(|t| constant_time_eq(t, expected))
    {
        return true;
    }

    if QUERY_TOKEN_PATHS.contains(&path) {
        return query
            .and_then(token_from_query)
            .is_some_and(|t| constant_time_eq(t, expected));
    }

    false
}

/// Auth middleware — rejects requests without a valid Bearer token.
/// SSE endpoints listed in [`QUERY_TOKEN_PATHS`] accept either Bearer header or `?token=`.
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

/// Persist the current session's bearer token and bound HTTP port into
/// `~/.colima-ui/` so the local `cui` CLI and host-side agents can authenticate
/// without any HTTP token endpoint.
pub fn persist_session_credentials(bound_port: u16) -> io::Result<()> {
    persist_credentials_to_dir(
        &crate::path_util::app_data_dir(),
        &get_api_token(),
        bound_port,
    )
}

/// Remove `~/.colima-ui/api_token` and `~/.colima-ui/api_port` on shutdown if
/// and only if `api_token` still belongs to this process.
pub fn cleanup_session_credentials() {
    cleanup_credentials_in_dir(&crate::path_util::app_data_dir(), &get_api_token());
}

fn persist_credentials_to_dir(dir: &Path, token: &str, bound_port: u16) -> io::Result<()> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refusing symlink or non-directory credential directory",
                ));
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(dir)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(dir)?;
            }
        }
        Err(e) => return Err(e),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(dir)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing symlink or non-directory credential directory",
            ));
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }

    write_secret_file_atomic(dir, TOKEN_FILE_NAME, token.as_bytes())?;
    write_secret_file_atomic(dir, PORT_FILE_NAME, bound_port.to_string().as_bytes())
}

fn write_and_sync(mut file: std::fs::File, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;
    file.write_all(contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.sync_all()
}

fn write_secret_file_atomic(dir: &Path, filename: &str, contents: &[u8]) -> io::Result<()> {
    let dest = dir.join(filename);
    match std::fs::symlink_metadata(&dest) {
        Ok(meta) if meta.file_type().is_symlink() || meta.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing symlink or directory target",
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let mut rand_bytes = [0u8; 8];
    rand::fill(&mut rand_bytes);
    let rand_hex: String = rand_bytes.iter().map(|b| format!("{b:02x}")).collect();
    let tmp_path = dir.join(format!(".{filename}.{}.{rand_hex}.tmp", std::process::id()));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }

    let file = opts.open(&tmp_path)?;
    if let Err(e) = write_and_sync(file, contents) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp_path, &dest) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(&dest)?;
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "post-rename target is not a regular file",
            ));
        }
        if meta.permissions().mode() & 0o777 != 0o600 {
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
        }
    }

    Ok(())
}

fn cleanup_credentials_in_dir(dir: &Path, expected_token: &str) {
    let Ok(dir_meta) = std::fs::symlink_metadata(dir) else {
        return;
    };
    if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
        return;
    }

    let token_path = dir.join(TOKEN_FILE_NAME);
    let Ok(token_meta) = std::fs::symlink_metadata(&token_path) else {
        return;
    };
    if token_meta.file_type().is_symlink() || !token_meta.file_type().is_file() {
        return;
    }

    let existing = {
        use std::io::Read;
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.custom_flags(libc::O_NOFOLLOW);
        }
        let Ok(mut file) = opts.open(&token_path) else {
            return;
        };
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            return;
        }
        buf
    };

    if !constant_time_eq(existing.trim(), expected_token) {
        return;
    }

    let _ = std::fs::remove_file(&token_path);

    let port_path = dir.join(PORT_FILE_NAME);
    if std::fs::symlink_metadata(&port_path)
        .is_ok_and(|m| !m.file_type().is_symlink() && m.file_type().is_file())
    {
        let _ = std::fs::remove_file(&port_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const TOKEN: &str = "colima-0123456789abcdef";

    struct TempTestDir(PathBuf);

    impl TempTestDir {
        fn new(label: &str) -> Self {
            let mut bytes = [0u8; 8];
            rand::fill(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir().join(format!(
                "colima-ui-auth-test-{label}-{}-{hex}",
                std::process::id()
            ));
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            if let Ok(meta) = std::fs::symlink_metadata(&self.0) {
                if meta.file_type().is_symlink() || meta.file_type().is_file() {
                    let _ = std::fs::remove_file(&self.0);
                } else if meta.is_dir() {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
        }
    }

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
    fn header_and_query_token_both_work_on_query_token_paths() {
        let header = format!("Bearer {}", TOKEN);
        let q = format!("token={}", TOKEN);
        assert!(is_authorized("/api/events", None, Some(&header), TOKEN));
        assert!(is_authorized(
            "/api/k8s/pods/logs/stream",
            Some("namespace=default&pod=nginx"),
            Some(&header),
            TOKEN
        ));
        assert!(is_authorized("/api/events", Some(&q), None, TOKEN));
        assert!(!is_authorized(
            "/api/events",
            Some("token=wrong"),
            Some("Bearer wrong"),
            TOKEN
        ));
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

    #[test]
    fn persists_credentials_with_strict_0700_and_0600_permissions() {
        let dir = TempTestDir::new("perms");
        persist_credentials_to_dir(dir.path(), TOKEN, 11423)
            .expect("initial credential persistence must succeed");

        let token_path = dir.path().join(TOKEN_FILE_NAME);
        let port_path = dir.path().join(PORT_FILE_NAME);
        assert_eq!(std::fs::read_to_string(&token_path).unwrap(), TOKEN);
        assert_eq!(std::fs::read_to_string(&port_path).unwrap(), "11423");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_mode = std::fs::symlink_metadata(dir.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let token_mode = std::fs::symlink_metadata(&token_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let port_mode = std::fs::symlink_metadata(&port_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700, "credential dir must be 0700");
            assert_eq!(token_mode, 0o600, "api_token must be 0600");
            assert_eq!(port_mode, 0o600, "api_port must be 0600");
        }

        // Overwriting an existing session's credentials must also replace atomically
        // and keep strict permissions.
        let second_token = "colima-fedcba9876543210";
        persist_credentials_to_dir(dir.path(), second_token, 11424)
            .expect("atomic overwrite must succeed");
        assert_eq!(std::fs::read_to_string(&token_path).unwrap(), second_token);
        assert_eq!(std::fs::read_to_string(&port_path).unwrap(), "11424");
    }

    #[test]
    #[cfg(unix)]
    fn refuses_symlinked_directory_and_symlinked_target_files() {
        use std::os::unix::fs::symlink;

        // 1. Symlinked credential directory must be rejected.
        let real_dir = TempTestDir::new("real-dir");
        std::fs::create_dir_all(real_dir.path()).unwrap();
        let symlink_dir = TempTestDir::new("symlink-dir");
        symlink(real_dir.path(), symlink_dir.path()).unwrap();

        let err = persist_credentials_to_dir(symlink_dir.path(), TOKEN, 11420)
            .expect_err("symlinked credential dir must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(!real_dir.path().join(TOKEN_FILE_NAME).exists());

        // 2. Symlinked api_token pointing at a victim file must be rejected without
        //    clobbering the victim file.
        let victim_dir = TempTestDir::new("victim");
        std::fs::create_dir_all(victim_dir.path()).unwrap();
        let victim_file = victim_dir.path().join("victim.txt");
        std::fs::write(&victim_file, "untouched-victim-content").unwrap();

        let cred_dir = TempTestDir::new("symlink-target");
        std::fs::create_dir_all(cred_dir.path()).unwrap();
        symlink(&victim_file, cred_dir.path().join(TOKEN_FILE_NAME)).unwrap();

        let err = persist_credentials_to_dir(cred_dir.path(), TOKEN, 11420)
            .expect_err("symlinked api_token target must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read_to_string(&victim_file).unwrap(),
            "untouched-victim-content"
        );

        // Cleanup must also refuse to follow a symlinked api_token even if the
        // victim file happens to contain the matching token.
        let token_victim = victim_dir.path().join("token-victim.txt");
        std::fs::write(&token_victim, TOKEN).unwrap();
        std::fs::remove_file(cred_dir.path().join(TOKEN_FILE_NAME)).unwrap();
        symlink(&token_victim, cred_dir.path().join(TOKEN_FILE_NAME)).unwrap();
        cleanup_credentials_in_dir(cred_dir.path(), TOKEN);
        assert!(
            token_victim.exists(),
            "cleanup must never unlink the target of a symlink"
        );
    }

    #[test]
    fn cleanup_removes_owned_credentials_and_preserves_newer_instance_credentials() {
        let dir = TempTestDir::new("cleanup");
        let newer_token = "colima-newer-session-99999999";
        persist_credentials_to_dir(dir.path(), newer_token, 11421).unwrap();

        // Older exiting instance has a different token — must leave both files intact.
        cleanup_credentials_in_dir(dir.path(), TOKEN);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(TOKEN_FILE_NAME)).unwrap(),
            newer_token
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(PORT_FILE_NAME)).unwrap(),
            "11421"
        );

        // Matching owner exits — both files must be removed.
        cleanup_credentials_in_dir(dir.path(), newer_token);
        assert!(!dir.path().join(TOKEN_FILE_NAME).exists());
        assert!(!dir.path().join(PORT_FILE_NAME).exists());
    }
}
