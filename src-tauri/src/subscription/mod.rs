//! Local entitlement state for the open-source edition.
//!
//! All features are unlocked locally in the open-source build:
//! [`subscription_state`] always reports `entitled: true` with no external
//! billing lookup, cloud token, or remote service dependency.
//!
//! The offline cache (`subscription.json`) and IPC/HTTP state commands remain in
//! place so callers and persisted state records keep a stable contract across
//! restarts without ever consulting the network.
//!
//! # What must stay true
//!
//! Entitlement evaluation is purely local and offline. Losing network access
//! never downgrades features or surfaces an error to the UI.

pub mod cache;

use serde::{Deserialize, Serialize};

use cache::CachedSubscription;

/// Entitlement as the UI needs it.
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionState {
    /// Whether Pro is entitled right now — the only field a gate may read.
    pub entitled: bool,
    /// Whether any subscription record exists on this machine at all.
    pub known: bool,
    /// "pro" | "pro_teams" | null. **Display only.**
    pub tier: Option<String>,
    pub seats_total: Option<i64>,
    pub seats_claimed: Option<i64>,
    pub expires_at: Option<String>,
    pub last_checked_at: Option<String>,
}

impl SubscriptionState {
    fn none() -> Self {
        SubscriptionState {
            entitled: false,
            known: false,
            tier: None,
            seats_total: None,
            seats_claimed: None,
            expires_at: None,
            last_checked_at: None,
        }
    }

    /// Judge a cached record for whoever is signed in.
    ///
    /// `user_id` is the optional caller user identifier, or `None` when nobody
    /// is signed in — which still counts as entitled, because signing out must
    /// not revoke local state. A *different* user gets nothing from this record.
    fn from_cache(cache: Option<&CachedSubscription>, user_id: Option<&str>) -> Self {
        let Some(c) = cache else {
            return Self::none();
        };

        let entitled = c.is_entitled_for(user_id, chrono::Utc::now());

        // A record belonging to someone else is reported as entirely absent
        // rather than "known but not entitled": as far as this user is
        // concerned, there is no subscription on this machine, and the UI should
        // offer them the plans rather than explain a stranger's expiry date.
        let belongs_to_caller = user_id.is_none_or(|uid| uid == c.user_id);
        if !belongs_to_caller {
            return Self::none();
        }

        SubscriptionState {
            entitled,
            known: true,
            tier: Some(c.tier.clone()),
            seats_total: c.seats_total,
            seats_claimed: c.seats_claimed,
            expires_at: c.expires_at.clone(),
            last_checked_at: c.last_checked_at.clone(),
        }
    }
}

/// What the frontend sends after a successful call to the entitlement oracle.
///
/// Deliberately mirrors the oracle's response so the frontend does no
/// reshaping — a translation layer between the two is a place for the two
/// meanings of "entitled" to drift apart.
#[derive(Debug, Clone, Deserialize)]
pub struct EntitlementPayload {
    pub user_id: String,
    pub entitled: bool,
    pub tier: String,
    pub seats_total: Option<i64>,
    pub seats_claimed: Option<i64>,
    pub expires_at: Option<String>,
}

static ACTIVE_USER_ID: std::sync::LazyLock<std::sync::Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Currently signed-in user id as last reported by the frontend, or `None` when
/// signed out / before any session check has run.
pub fn active_user_id() -> Option<String> {
    ACTIVE_USER_ID.lock().ok().and_then(|g| g.clone())
}

fn set_active_user_id(user_id: Option<&str>) {
    let next = user_id.map(str::to_string);
    let changed = if let Ok(mut guard) = ACTIVE_USER_ID.lock() {
        let diff = *guard != next;
        *guard = next;
        diff
    } else {
        false
    };
    if changed {
        crate::commands::metrics_store::invalidate_entitlement_cache();
    }
}

// ===== Tauri commands =====

fn with_open_source_entitlement(mut state: SubscriptionState) -> SubscriptionState {
    state.entitled = true;
    state
}

/// Current entitlement, evaluated locally offline.
///
/// In the open-source edition all features are unlocked locally (`entitled` is
/// always `true`), while any cached metadata for `user_id` is preserved for
/// display compatibility.
#[tauri::command]
pub async fn subscription_state(user_id: Option<String>) -> SubscriptionState {
    set_active_user_id(user_id.as_deref());
    with_open_source_entitlement(SubscriptionState::from_cache(
        cache::load().as_ref(),
        user_id.as_deref(),
    ))
}

/// Persist what the oracle returned, then report the resulting state.
///
/// Called only after a *successful* oracle response. A failed call must not
/// reach here — writing "not entitled" because the network was down is exactly
/// the downgrade this design forbids.
#[tauri::command]
pub async fn subscription_store(
    payload: EntitlementPayload,
) -> Result<SubscriptionState, crate::error::ColimaError> {
    set_active_user_id(Some(payload.user_id.as_str()));
    let record = CachedSubscription {
        user_id: payload.user_id.clone(),
        entitled: payload.entitled,
        tier: payload.tier,
        seats_total: payload.seats_total,
        seats_claimed: payload.seats_claimed,
        expires_at: payload.expires_at,
        last_checked_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    cache::store(&record)?;

    // The record just changed, so the cached answer is stale by definition.
    crate::commands::metrics_store::invalidate_entitlement_cache();

    // An upgrade has to take effect now, not at the next launch. Lapsing
    // already stops writing within seconds — the writer re-reads entitlement on
    // every batch — but the other direction had no path at all: `Pro` was only
    // ever attached during start-up, so somebody who paid mid-session recorded
    // nothing until they happened to restart the app, with nothing on screen to
    // explain the gap in their history.
    //
    // No-op when a writer is already attached, and when the payload is not
    // entitled.
    crate::commands::metrics_store::start_if_entitled();

    Ok(with_open_source_entitlement(SubscriptionState::from_cache(
        Some(&record),
        Some(payload.user_id.as_str()),
    )))
}

/// Erase the local record.
///
/// Not called on sign-out — that deliberately leaves entitlement running until
/// expiry. This exists for support ("clear my subscription cache and re-check")
/// and for tests.
#[tauri::command]
pub async fn subscription_clear() -> Result<SubscriptionState, crate::error::ColimaError> {
    cache::clear()?;
    Ok(with_open_source_entitlement(SubscriptionState::none()))
}

// ===== HTTP handlers (browser mode) =====

use axum::http::StatusCode;
use axum::Json;

use crate::helpers::{err, ok, ApiResponse};

#[derive(Deserialize)]
pub struct StateQuery {
    pub user_id: Option<String>,
}

pub async fn api_subscription_state(
    axum::extract::Query(q): axum::extract::Query<StateQuery>,
) -> (StatusCode, Json<ApiResponse<SubscriptionState>>) {
    ok(subscription_state(q.user_id).await)
}

pub async fn api_subscription_store(
    Json(body): Json<EntitlementPayload>,
) -> (StatusCode, Json<ApiResponse<SubscriptionState>>) {
    match subscription_store(body).await {
        Ok(s) => ok(s),
        Err(e) => err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(user: &str, entitled: bool, expires: &str) -> CachedSubscription {
        CachedSubscription {
            user_id: user.into(),
            entitled,
            tier: "pro".into(),
            seats_total: Some(1),
            seats_claimed: Some(1),
            expires_at: Some(expires.into()),
            last_checked_at: None,
        }
    }

    #[test]
    fn no_cache_is_unknown_and_unentitled() {
        let s = SubscriptionState::from_cache(None, Some("user-a"));
        assert!(!s.entitled);
        assert!(!s.known);
    }

    #[test]
    fn matching_user_with_live_subscription_is_entitled() {
        let r = rec("user-a", true, "2030-01-01T00:00:00Z");
        let s = SubscriptionState::from_cache(Some(&r), Some("user-a"));
        assert!(s.entitled);
        assert!(s.known);
        assert_eq!(s.tier.as_deref(), Some("pro"));
    }

    /// Signing out must not revoke Pro — the decided behavior.
    #[test]
    fn signed_out_keeps_entitlement() {
        let r = rec("user-a", true, "2030-01-01T00:00:00Z");
        assert!(SubscriptionState::from_cache(Some(&r), None).entitled);
    }

    /// Sign out, sign in as somebody else, and the previous user's Pro must not
    /// come with it. Reported as "no subscription here", not as a foreign one.
    #[test]
    fn other_user_sees_no_subscription_at_all() {
        let r = rec("user-a", true, "2030-01-01T00:00:00Z");
        let s = SubscriptionState::from_cache(Some(&r), Some("user-b"));
        assert!(!s.entitled);
        assert!(!s.known, "must not leak that someone else's record exists");
        assert!(s.expires_at.is_none());
    }

    #[test]
    fn expired_cache_is_known_but_not_entitled() {
        let r = rec("user-a", true, "2025-01-01T00:00:00Z");
        let s = SubscriptionState::from_cache(Some(&r), Some("user-a"));
        assert!(!s.entitled);
        assert!(s.known, "the UI should still be able to say when it lapsed");
    }

    /// The whole point of the module: nothing here consults an external billing
    /// service, an account session, or the network. Cache evaluation is a pure
    /// function of the cached record, the user id, and the clock.
    #[test]
    fn entitlement_is_offline_only() {
        let r = rec("user-a", true, "2030-01-01T00:00:00Z");
        for _ in 0..3 {
            assert!(SubscriptionState::from_cache(Some(&r), Some("user-a")).entitled);
        }
    }

    #[test]
    fn open_source_entitlement_preserved_across_store_and_clear_states() {
        let cleared = with_open_source_entitlement(SubscriptionState::none());
        assert!(cleared.entitled);
        assert!(!cleared.known);

        let lapsed = rec("user-a", false, "2020-01-01T00:00:00Z");
        let stored = with_open_source_entitlement(SubscriptionState::from_cache(
            Some(&lapsed),
            Some("user-a"),
        ));
        assert!(stored.entitled);
        assert!(stored.known);
    }
}
