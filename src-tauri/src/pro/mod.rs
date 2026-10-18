//! The MIT-side boundary to the Pro sidecar.
//!
//! Everything here ships in the open-source core. It contains the *client* half
//! of the boundary — how to find the sidecar, hand-shake with it, and report
//! its status — but no Pro feature logic and no hardcoded list of what Pro can
//! do. The sidecar declares its own capabilities at handshake time.
//!
//! Deferred until the license format from the chosen MoR is known (Phase 2):
//! `cached_state` — the signed, durable record of "this user has paid" that lets
//! the core keep showing Pro identity even when the sidecar is missing. Until
//! then, [`ProStatus`] is derived live from the handshake only.

pub mod bridge;
pub mod protocol;

use serde::{Deserialize, Serialize};

/// What the core knows about Pro right now.
///
/// The default and overwhelmingly common state today is [`ProStatus::Free`]:
/// no sidecar is bundled yet, so the core runs the full free product with no
/// error and no nagging.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProStatus {
    /// No sidecar present, or it could not be reached. Full Free experience.
    Free,
    /// Sidecar present, license active. These capabilities may be used.
    Active { capabilities: Vec<String> },
    /// Sidecar present but its protocol version does not match the core.
    ///
    /// Critically NOT the same as Free: a paying user whose Pro component is
    /// stale must see "update your Pro component", never "you are not a
    /// customer". The UI keys off this variant to say the right thing.
    NeedsUpdate,
    /// Sidecar present and reachable, but it reports the license is not active
    /// (expired, not yet activated). Free features stay fully available.
    LicenseInactive,
}

impl ProStatus {
    /// Whether a named capability is currently usable. Anything other than an
    /// active license with that capability declared returns false, so callers
    /// can gate with a single check.
    pub fn has_capability(&self, capability: &str) -> bool {
        matches!(self, ProStatus::Active { capabilities } if capabilities.iter().any(|c| c == capability))
    }
}

/// Report Pro status to the UI. Cheap to call: it re-runs sidecar detection,
/// which short-circuits to [`ProStatus::Free`] the moment no socket is found.
///
/// Infallible by design — the front end always gets a status, never an error,
/// so a missing sidecar can never surface as a broken call.
#[tauri::command]
pub async fn pro_status() -> ProStatus {
    bridge::detect().await
}

/// REST equivalent of [`pro_status`] for browser mode. Kept next to the command
/// so the Pro boundary stays in one module rather than leaking into `routes/`.
pub async fn api_pro_status() -> (
    axum::http::StatusCode,
    axum::response::Json<crate::api_server::ApiResponse<ProStatus>>,
) {
    crate::api_server::ok(bridge::detect().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_has_no_capabilities() {
        assert!(!ProStatus::Free.has_capability("compose.autofix"));
    }

    #[test]
    fn needs_update_has_no_capabilities_but_is_distinct_from_free() {
        assert!(!ProStatus::NeedsUpdate.has_capability("compose.autofix"));
        assert_ne!(ProStatus::NeedsUpdate, ProStatus::Free);
    }

    #[test]
    fn active_gates_on_declared_capability_only() {
        let s = ProStatus::Active { capabilities: vec!["compose.autofix".into()] };
        assert!(s.has_capability("compose.autofix"));
        assert!(!s.has_capability("dockerfile.optimize"));
    }

    #[test]
    fn status_round_trips_for_frontend() {
        let s = ProStatus::Active { capabilities: vec!["compose.autofix".into()] };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"state\":\"active\""));
        let back: ProStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
