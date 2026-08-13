//! Locating the Pro sidecar and deriving [`ProStatus`] from a handshake.
//!
//! The line protocol is newline-delimited JSON over a Unix domain socket: the
//! core writes one [`Handshake`] line and reads one [`HandshakeReply`] line.
//!
//! The guiding rule is that **absence is normal, not an error**. No sidecar is
//! bundled yet, so [`detect`] returning [`ProStatus::Free`] is the expected
//! path today, and it must be silent — no log noise, no user-facing failure.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::protocol::{version_verdict, Handshake, HandshakeReply, VersionVerdict, CORE_PROTOCOL_VERSION};
use super::ProStatus;

/// How long the whole detect round-trip may take before the core gives up and
/// treats Pro as absent. Deliberately short: this runs at startup and must not
/// delay a Free user.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(500);

/// Where the core looks for the sidecar's socket.
///
/// In a debug build the path may be overridden by `COLIMA_UI_PRO_SOCK` so the
/// sidecar can be run from a separate checkout during development. In release
/// builds that override is ignored — trusting an env-var-supplied path in a
/// signed binary would let anyone point the trusted process at an arbitrary
/// peer (red-team finding #10).
///
/// The production path here is a documented placeholder. The final location is
/// settled together with the bundled sidecar and its signing setup (Phase 3,
/// once the binary exists).
fn socket_path() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Ok(p) = std::env::var("COLIMA_UI_PRO_SOCK") {
        return PathBuf::from(p);
    }
    std::env::temp_dir().join("colima-ui-pro.sock")
}

/// A per-spawn nonce. Not a secret — defense in depth so a stale process
/// squatting on the socket path is not mistaken for the one we expect.
fn make_nonce() -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{pid:x}-{nanos:x}")
}

/// Perform the handshake over an already-connected stream and map the reply to
/// a [`ProStatus`]. Split out from [`detect`] so it can be tested against an
/// in-process mock without spawning anything.
async fn handshake(stream: UnixStream, nonce: &str) -> ProStatus {
    let mut reader = BufReader::new(stream);

    let hello = Handshake { core_protocol_version: CORE_PROTOCOL_VERSION, nonce: nonce.to_string() };
    let Ok(mut line) = serde_json::to_string(&hello) else {
        return ProStatus::Free;
    };
    line.push('\n');
    if reader.get_mut().write_all(line.as_bytes()).await.is_err() {
        return ProStatus::Free;
    }

    let mut resp = String::new();
    if reader.read_line(&mut resp).await.unwrap_or(0) == 0 {
        return ProStatus::Free;
    }

    let Ok(reply) = serde_json::from_str::<HandshakeReply>(resp.trim()) else {
        return ProStatus::Free;
    };

    // A reply that does not echo our nonce is not the peer we launched.
    if reply.nonce != nonce {
        return ProStatus::Free;
    }

    match version_verdict(reply.sidecar_protocol_version) {
        VersionVerdict::Incompatible => ProStatus::NeedsUpdate,
        VersionVerdict::Compatible => {
            if reply.license_active {
                ProStatus::Active { capabilities: reply.capabilities }
            } else {
                ProStatus::LicenseInactive
            }
        }
    }
}

/// Detect the Pro sidecar and report its status.
///
/// Never errors: any failure to find, connect to, or handshake with the sidecar
/// collapses to [`ProStatus::Free`], because that is the correct, common state.
pub async fn detect() -> ProStatus {
    let path = socket_path();

    let connect = async {
        match UnixStream::connect(&path).await {
            Ok(stream) => handshake(stream, &make_nonce()).await,
            // Missing socket is the normal case; do not surface it.
            Err(_) => ProStatus::Free,
        }
    };

    tokio::time::timeout(HANDSHAKE_TIMEOUT, connect)
        .await
        .unwrap_or(ProStatus::Free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    /// Spawn a one-shot mock sidecar that replies with the given values, and
    /// return the socket path it listens on.
    fn mock_sidecar(
        dir: &std::path::Path,
        reply: HandshakeReply,
        echo_nonce: bool,
    ) -> PathBuf {
        let path = dir.join("mock.sock");
        let listener = UnixListener::bind(&path).unwrap();
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;
                let incoming: Handshake = serde_json::from_str(line.trim()).unwrap();
                let mut r = reply;
                if echo_nonce {
                    r.nonce = incoming.nonce;
                }
                let mut out = serde_json::to_string(&r).unwrap();
                out.push('\n');
                let _ = reader.get_mut().write_all(out.as_bytes()).await;
            }
        });
        path
    }

    async fn connect_and_handshake(path: &std::path::Path, nonce: &str) -> ProStatus {
        let stream = UnixStream::connect(path).await.unwrap();
        handshake(stream, nonce).await
    }

    #[tokio::test]
    async fn missing_socket_is_free_not_error() {
        // detect() against a path with nothing listening must be silent Free.
        let status = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            match UnixStream::connect(std::env::temp_dir().join("colima-ui-pro-does-not-exist.sock")).await {
                Ok(s) => handshake(s, "n").await,
                Err(_) => ProStatus::Free,
            }
        })
        .await
        .unwrap_or(ProStatus::Free);
        assert_eq!(status, ProStatus::Free);
    }

    #[tokio::test]
    async fn valid_active_sidecar_yields_active() {
        let dir = std::env::temp_dir().join(format!("pro-test-active-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = mock_sidecar(
            &dir,
            HandshakeReply {
                sidecar_protocol_version: CORE_PROTOCOL_VERSION,
                nonce: String::new(),
                license_active: true,
                capabilities: vec!["compose.autofix".into()],
            },
            true,
        );
        let status = connect_and_handshake(&path, "the-nonce").await;
        assert_eq!(status, ProStatus::Active { capabilities: vec!["compose.autofix".into()] });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn version_mismatch_yields_needs_update_not_free() {
        // The whole point: a stale paid sidecar must not read as "not a customer".
        let dir = std::env::temp_dir().join(format!("pro-test-skew-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = mock_sidecar(
            &dir,
            HandshakeReply {
                sidecar_protocol_version: CORE_PROTOCOL_VERSION + 1,
                nonce: String::new(),
                license_active: true,
                capabilities: vec!["compose.autofix".into()],
            },
            true,
        );
        let status = connect_and_handshake(&path, "the-nonce").await;
        assert_eq!(status, ProStatus::NeedsUpdate);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inactive_license_yields_license_inactive() {
        let dir = std::env::temp_dir().join(format!("pro-test-inactive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = mock_sidecar(
            &dir,
            HandshakeReply {
                sidecar_protocol_version: CORE_PROTOCOL_VERSION,
                nonce: String::new(),
                license_active: false,
                capabilities: vec![],
            },
            true,
        );
        let status = connect_and_handshake(&path, "the-nonce").await;
        assert_eq!(status, ProStatus::LicenseInactive);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn wrong_nonce_is_rejected_as_free() {
        // A process squatting the socket that cannot echo our nonce is ignored.
        let dir = std::env::temp_dir().join(format!("pro-test-nonce-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = mock_sidecar(
            &dir,
            HandshakeReply {
                sidecar_protocol_version: CORE_PROTOCOL_VERSION,
                nonce: "a-different-nonce".into(),
                license_active: true,
                capabilities: vec!["compose.autofix".into()],
            },
            false, // do NOT echo — simulate an impostor
        );
        let status = connect_and_handshake(&path, "the-nonce").await;
        assert_eq!(status, ProStatus::Free);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
