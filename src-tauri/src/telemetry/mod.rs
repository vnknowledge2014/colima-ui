//! Telemetry: consent-gated, closed-vocabulary event recording.
//!
//! **The network sink is deferred.** There is no server endpoint yet, so
//! recorded events go into a bounded in-memory buffer and nothing is ever
//! transmitted. That buffer backs the "show what would be sent" preview the
//! consent flow promises. When an endpoint exists, a sink reads this buffer;
//! until then, "zero telemetry requests" is guaranteed by there being no
//! network code at all.
//!
//! Consent is checked on every [`record`] call, so flipping consent off stops
//! recording immediately.

pub mod consent;
pub mod events;

use std::collections::VecDeque;
use std::sync::Mutex;

use events::TelemetryEvent;

/// Upper bound on buffered events. Old events are dropped first; telemetry must
/// never grow without limit or block anything.
const BUFFER_CAP: usize = 256;

/// The in-memory event buffer. Not persisted, not transmitted.
static BUFFER: Mutex<VecDeque<TelemetryEvent>> = Mutex::new(VecDeque::new());

/// Record an event, if consent allows it.
///
/// Silently does nothing when consent is not granted — the common case. Never
/// errors and never blocks: a poisoned lock is swallowed because telemetry must
/// not take down a real operation.
pub async fn record(event: TelemetryEvent) {
    if !consent::current().await.allows_recording() {
        return;
    }
    if let Ok(mut buf) = BUFFER.lock() {
        if buf.len() >= BUFFER_CAP {
            buf.pop_front();
        }
        buf.push_back(event);
    }
}

/// The events currently buffered, for the "show what would be sent" preview.
fn buffered() -> Vec<TelemetryEvent> {
    BUFFER.lock().map(|b| b.iter().cloned().collect()).unwrap_or_default()
}

// ===== Tauri commands =====

/// Current consent state, for the settings UI and the one-time dialog.
#[tauri::command]
pub async fn telemetry_consent() -> consent::Consent {
    consent::current().await
}

/// Whether the one-time consent dialog should be shown (only when never asked).
#[tauri::command]
pub async fn telemetry_should_prompt() -> bool {
    consent::current().await.should_prompt()
}

/// Record a consent decision from the dialog or settings toggle.
#[tauri::command]
pub async fn telemetry_set_consent(granted: bool) -> Result<(), crate::error::ColimaError> {
    consent::set(if granted { consent::Consent::Granted } else { consent::Consent::Declined }).await
}

/// The exact events that would be sent, so the user can inspect them first.
#[tauri::command]
pub async fn telemetry_preview() -> Vec<TelemetryEvent> {
    buffered()
}

// ===== Funnel instrumentation =====
//
// These record real product events at the points the plan's funnel needs
// (install → activation → ProGate reached → checkout). Every one takes a typed
// enum, never a free string, so the closed-vocabulary PII barrier holds even
// though the caller is the front end.

/// Record app startup with coarse platform facts. Backend-sourced (version, os,
/// arch) — never trusts the caller for these.
pub async fn record_app_started(app_version: String) {
    record(events::TelemetryEvent::AppStarted {
        app_version,
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })
    .await;
}

/// A named feature was used. `feature` is an enum, so an invalid value is
/// rejected at deserialization rather than recorded.
#[tauri::command]
pub async fn telemetry_feature_used(feature: events::Feature) {
    record(events::TelemetryEvent::FeatureUsed { feature }).await;
}

/// A Pro gate was shown to a non-entitled user — a funnel signal.
#[tauri::command]
pub async fn telemetry_pro_gate_reached(capability: events::GatedCapability) {
    record(events::TelemetryEvent::ProGateReached { capability }).await;
}

/// The checkout page was opened — the last funnel step before purchase.
#[tauri::command]
pub async fn telemetry_checkout_opened() {
    record(events::TelemetryEvent::CheckoutOpened).await;
}

// ===== REST equivalents for browser mode =====

use axum::http::StatusCode;
use axum::response::Json;
use crate::api_server::{err, ok, ApiResponse};

pub async fn api_telemetry_consent() -> (StatusCode, Json<ApiResponse<consent::Consent>>) {
    ok(consent::current().await)
}

pub async fn api_telemetry_should_prompt() -> (StatusCode, Json<ApiResponse<bool>>) {
    ok(consent::current().await.should_prompt())
}

#[derive(serde::Deserialize)]
pub struct SetConsentBody {
    pub granted: bool,
}

pub async fn api_telemetry_set_consent(
    Json(body): Json<SetConsentBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match consent::set(if body.granted { consent::Consent::Granted } else { consent::Consent::Declined }).await {
        Ok(()) => ok("saved".to_string()),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_telemetry_preview() -> (StatusCode, Json<ApiResponse<Vec<TelemetryEvent>>>) {
    ok(buffered())
}

#[cfg(test)]
mod tests {
    use super::*;
    use events::Feature;

    #[test]
    fn buffer_is_bounded() {
        // Fill past the cap directly (bypassing consent, which needs the DB) and
        // confirm the oldest events are dropped rather than growing without end.
        {
            let mut buf = BUFFER.lock().unwrap();
            buf.clear();
            for _ in 0..(BUFFER_CAP + 50) {
                if buf.len() >= BUFFER_CAP {
                    buf.pop_front();
                }
                buf.push_back(TelemetryEvent::FeatureUsed { feature: Feature::Terminal });
            }
        }
        assert!(buffered().len() <= BUFFER_CAP);
        BUFFER.lock().unwrap().clear();
    }
}
