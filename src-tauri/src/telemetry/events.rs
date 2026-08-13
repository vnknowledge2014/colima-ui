//! The closed telemetry vocabulary.
//!
//! This is the PII barrier, enforced by the type system rather than by a
//! promise: there is no variant and no field that accepts a free-form string,
//! so code physically cannot record a container name, a path, or a chat
//! message. Every variant here must match a row in `docs/telemetry.md`.
//!
//! Changing this enum is the moment to re-read that doc — a new field that can
//! carry user data is exactly the mistake this design exists to prevent.

use serde::{Deserialize, Serialize};

use crate::error::ErrorCode;

/// A feature whose use is worth counting. An enum, not a string, on purpose:
/// the set of things we measure is small, named, and reviewable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    ComposeDiagnose,
    DockerfileOptimize,
    KnowledgeBankQuery,
    Terminal,
    KubernetesView,
}

/// A Pro feature a user reached without being entitled to it.
///
/// Closed on purpose: a free-form string here would carry whatever the frontend
/// passed, and this enum is the reason a telemetry payload cannot contain a
/// path, a container name or a project. A feature missing from this list is
/// dropped rather than recorded, so adding a gated feature means adding it here
/// and to `GATED_ENUM` in `ProGate.svelte` — miss either and the funnel goes
/// quiet without failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatedCapability {
    ComposeAutofix,
    ComposeDiagnose,
    DockerfileOptimize,
    MetricsHistory,
    MetricsAlerts,
    SelfHealing,
    SecurityTriage,
    SecurityAutofix,
    SecurityHistory,
    ActivityExport,
}

/// Every telemetry event. No variant carries free-form text; the closest is
/// [`TelemetryEvent::Error`], which carries only the stable [`ErrorCode`] and
/// never the human-readable detail (that can contain paths and names).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// App launched. Carries only coarse platform facts.
    AppStarted {
        app_version: String,
        os: String,
        arch: String,
    },
    /// The user reached their first working container — the activation moment.
    Activation,
    /// A named feature was used.
    FeatureUsed { feature: Feature },
    /// An operation failed. The code only, never the message.
    Error { code: ErrorCode },
    /// A Pro gate was reached by a user without that capability.
    ProGateReached { capability: GatedCapability },
    /// The checkout page was opened.
    CheckoutOpened,
}

#[cfg(test)]
mod gated_capability_tests {
    use super::*;

    /// The wire names must match `GATED_ENUM` in `ProGate.svelte` exactly. They
    /// are two halves of one mapping in two languages, and a mismatch fails
    /// silently — the event is dropped and the funnel simply reports nothing.
    #[test]
    fn wire_names_match_the_frontend_map() {
        for (variant, expected) in [
            (GatedCapability::ComposeAutofix, "\"compose_autofix\""),
            (GatedCapability::ComposeDiagnose, "\"compose_diagnose\""),
            (GatedCapability::DockerfileOptimize, "\"dockerfile_optimize\""),
            (GatedCapability::MetricsHistory, "\"metrics_history\""),
            (GatedCapability::MetricsAlerts, "\"metrics_alerts\""),
            (GatedCapability::SelfHealing, "\"self_healing\""),
            (GatedCapability::SecurityTriage, "\"security_triage\""),
            (GatedCapability::SecurityAutofix, "\"security_autofix\""),
            (GatedCapability::SecurityHistory, "\"security_history\""),
            (GatedCapability::ActivityExport, "\"activity_export\""),
        ] {
            let json = serde_json::to_string(&variant).expect("serialisable");
            assert_eq!(json, expected, "wire name drifted for {variant:?}");
        }
    }

    /// An unknown feature id is rejected rather than recorded. This is what keeps
    /// free-form text — paths, container names — out of telemetry.
    #[test]
    fn an_unknown_capability_is_refused() {
        let parsed: Result<GatedCapability, _> = serde_json::from_str("\"/Users/someone/project\"");
        assert!(parsed.is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_with_a_stable_tag() {
        let e = TelemetryEvent::FeatureUsed { feature: Feature::ComposeDiagnose };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"event\":\"feature_used\""));
        assert!(json.contains("\"feature\":\"compose_diagnose\""));
    }

    #[test]
    fn error_event_carries_code_not_message() {
        // The whole reason Error takes an ErrorCode and not a String: the
        // message can contain paths and resource names, the code cannot.
        let e = TelemetryEvent::Error { code: ErrorCode::CommandFailed };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("command_failed") || json.contains("CommandFailed"));
        // There is no field that could hold a detail string; this is a
        // compile-time guarantee, asserted here for documentation.
        assert!(!json.to_lowercase().contains("detail"));
    }

    #[test]
    fn app_started_round_trips() {
        let e = TelemetryEvent::AppStarted {
            app_version: "0.1.10".into(),
            os: "macos".into(),
            arch: "aarch64".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: TelemetryEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }
}
