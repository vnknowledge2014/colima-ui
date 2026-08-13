//! Wire protocol between the MIT core and the Pro sidecar.
//!
//! This lives in the MIT core on purpose: the private Pro repo imports the same
//! types so both ends agree on the format. It contains **no Pro logic** — only
//! the shape of the messages and the rules for what to do when the two ends
//! disagree on version.
//!
//! The message set is deliberately shaped by a payload that already exists —
//! the compose diagnosis from `commands::compose_diagnose` — rather than
//! designed in the abstract. `Invoke`/`InvokeResult` carry opaque JSON so the
//! core never needs to understand a capability's payload; it only routes it.

use serde::{Deserialize, Serialize};

/// The protocol version the core speaks.
///
/// Bumped only on a breaking change to the message set. The handshake compares
/// this against the sidecar's value; see [`VersionVerdict`].
pub const CORE_PROTOCOL_VERSION: u32 = 1;

/// Core → sidecar, first message after the socket connects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Handshake {
    pub core_protocol_version: u32,
    /// Random per-spawn value; the sidecar must echo it back so a stale process
    /// squatting on the socket path cannot impersonate the one we launched.
    pub nonce: String,
}

/// Sidecar → core, reply to [`Handshake`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeReply {
    pub sidecar_protocol_version: u32,
    /// Must equal the nonce the core sent, or the core rejects the connection.
    pub nonce: String,
    /// The sidecar's own assertion that the license is currently valid.
    ///
    /// This is not the durable source of truth — that is the signed cached
    /// state (deferred until the license format from the MoR is known). It is
    /// only what the live sidecar reports right now.
    pub license_active: bool,
    /// Capabilities the sidecar offers, e.g. `"compose.autofix"`. The core never
    /// hardcodes this list — it learns it here and gates UI on it.
    pub capabilities: Vec<String>,
}

/// Core → sidecar: run one capability with an opaque payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Invoke {
    pub capability: String,
    /// Raw JSON. The core does not parse it; only the capability's owner does.
    pub payload: serde_json::Value,
}

/// Sidecar → core: the result of an [`Invoke`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InvokeResult {
    Ok { data: serde_json::Value },
    Err { message: String },
}

/// What the core should do given the sidecar's protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionVerdict {
    /// Same major version — talk normally.
    Compatible,
    /// The two ends disagree on the message format. The core must NOT downgrade
    /// a paying user to Free; it shows "Pro component needs updating" instead.
    Incompatible,
}

/// Decide compatibility between the core and a sidecar.
///
/// Version 0 (or anything the core does not recognise) is treated as
/// incompatible rather than trusted. Bundle-together distribution makes a
/// mismatch rare, but a sidecar left behind by a partial update or restored
/// from backup can still appear, and stranding a paid customer is the exact
/// failure this guards against.
pub fn version_verdict(sidecar_version: u32) -> VersionVerdict {
    if sidecar_version == CORE_PROTOCOL_VERSION {
        VersionVerdict::Compatible
    } else {
        VersionVerdict::Incompatible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_version_is_compatible() {
        assert_eq!(version_verdict(CORE_PROTOCOL_VERSION), VersionVerdict::Compatible);
    }

    #[test]
    fn different_version_is_incompatible_not_trusted() {
        assert_eq!(version_verdict(CORE_PROTOCOL_VERSION + 1), VersionVerdict::Incompatible);
        assert_eq!(version_verdict(0), VersionVerdict::Incompatible);
    }

    #[test]
    fn handshake_round_trips_through_json() {
        let h = Handshake { core_protocol_version: 1, nonce: "abc123".into() };
        let json = serde_json::to_string(&h).unwrap();
        let back: Handshake = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn handshake_reply_round_trips_through_json() {
        let r = HandshakeReply {
            sidecar_protocol_version: 1,
            nonce: "abc123".into(),
            license_active: true,
            capabilities: vec!["compose.autofix".into(), "dockerfile.optimize".into()],
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: HandshakeReply = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn invoke_result_tag_is_stable() {
        // The private repo depends on these tag strings; a rename is a breaking
        // protocol change and must bump CORE_PROTOCOL_VERSION.
        let ok = serde_json::to_string(&InvokeResult::Ok { data: serde_json::json!({"x": 1}) }).unwrap();
        assert!(ok.contains("\"status\":\"ok\""));
        let err = serde_json::to_string(&InvokeResult::Err { message: "boom".into() }).unwrap();
        assert!(err.contains("\"status\":\"err\""));
    }

    #[test]
    fn invoke_payload_is_opaque_json() {
        // The core routes arbitrary capability payloads without understanding
        // them; a compose-diagnosis-shaped payload must survive untouched.
        let inv = Invoke {
            capability: "compose.autofix".into(),
            payload: serde_json::json!({ "file_path": "/proj/docker-compose.yml", "signature": "services.web additional properties" }),
        };
        let json = serde_json::to_string(&inv).unwrap();
        let back: Invoke = serde_json::from_str(&json).unwrap();
        assert_eq!(inv, back);
    }
}
