//! Local, offline-first record of the user's subscription entitlement.
//!
//! Replaces `license/cache.rs`. The persistence machinery is deliberately
//! unchanged — atomic write (tmp + fsync + rename), a `.bak` of the last good
//! copy, owner-only permissions — because a corrupted cache turning a paying
//! user into Free is the failure this file exists to prevent. Only the payload
//! is new.
//!
//! # Why a file and not the keychain
//!
//! The session tokens live in the macOS Keychain (`account_session.rs`), and it
//! is tempting to put this beside them. It would be wrong: that module has a
//! backend only on macOS, so a Linux user's entitlement would never persist and
//! every offline launch would be Free.
//!
//! The two are different kinds of thing. A token is a *credential* — stealing it
//! is account takeover. This is a *claim* — forging it grants Pro to the forger
//! and nobody else, which is the "polite fence, not DRM" position the project
//! already took for license keys.

use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The persisted entitlement, as returned by the entitlement oracle.
///
/// `user_id` is what makes sign-out safe: entitlement survives signing out, but
/// a *different* user signing in must not inherit it. See [`is_entitled_for`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedSubscription {
    /// The Supabase user this entitlement belongs to.
    pub user_id: String,
    /// What the oracle last said. Not re-derived here; this file only judges expiry.
    pub entitled: bool,
    /// "pro" | "pro_teams". **Display only** — no gate may branch on it.
    pub tier: String,
    pub seats_total: Option<i64>,
    pub seats_claimed: Option<i64>,
    /// RFC3339 UTC. When this passes, the app must re-check before staying Pro.
    pub expires_at: Option<String>,
    pub last_checked_at: Option<String>,
}

impl CachedSubscription {
    /// Whether this record still entitles Pro at `now`, judged entirely offline.
    ///
    /// A missing expiry fails **closed**. That differs from the old license
    /// cache, where `None` meant a perpetual key. A subscription always has a
    /// renewal date, so an absent one means the record is malformed rather than
    /// eternal — and guessing "eternal" would be guessing in the app's favour
    /// against the customer's payment status.
    pub fn is_entitled_at(&self, now: DateTime<Utc>) -> bool {
        if !self.entitled {
            return false;
        }
        match &self.expires_at {
            None => false,
            Some(s) => match DateTime::parse_from_rfc3339(s) {
                Ok(exp) => exp.with_timezone(&Utc) > now,
                // Unparseable expiry is treated as expired, not trusted.
                Err(_) => false,
            },
        }
    }

    /// Whether this record entitles Pro for whoever is signed in right now.
    ///
    /// Three cases, and the third is the one that matters:
    ///
    /// | Signed in as | Result |
    /// |---|---|
    /// | The same user | entitled per the record |
    /// | Nobody | entitled per the record — signing out does not revoke Pro |
    /// | A different user | **never** — they resolve their own entitlement |
    ///
    /// Without the third case, signing out and back in with a throwaway free
    /// account would show Pro: a bypass anyone would find, and a wrong answer
    /// shown to an honest user who simply switched accounts.
    pub fn is_entitled_for(&self, user_id: Option<&str>, now: DateTime<Utc>) -> bool {
        if let Some(uid) = user_id {
            if uid != self.user_id {
                return false;
            }
        }
        self.is_entitled_at(now)
    }
}

/// `~/.colima-ui/subscription.json`, beside the knowledge bank db.
fn subscription_path() -> PathBuf {
    crate::path_util::app_data_dir().join("subscription.json")
}

fn backup_path() -> PathBuf {
    let mut p = subscription_path();
    p.set_extension("json.bak");
    p
}

/// Write the record atomically, keeping a `.bak` of the previous good file.
pub fn store(sub: &CachedSubscription) -> Result<(), String> {
    let path = subscription_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;
    }

    if path.exists() {
        let _ = std::fs::copy(&path, backup_path());
    }

    let json = serde_json::to_string_pretty(sub).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");

    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
        f.write_all(json.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        f.sync_all().map_err(|e| format!("fsync: {e}"))?;
        set_owner_only(&f);
    }

    std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// Load the record, falling back to `.bak` if the primary is unreadable or
/// corrupt — a torn write must never read as "not subscribed".
pub fn load() -> Option<CachedSubscription> {
    if let Some(s) = read_file(&subscription_path()) {
        return Some(s);
    }
    read_file(&backup_path())
}

/// Remove the record. Used by support tooling, not by sign-out — signing out
/// deliberately leaves entitlement in place until it expires.
pub fn clear() -> Result<(), String> {
    let _ = std::fs::remove_file(subscription_path());
    let _ = std::fs::remove_file(backup_path());
    Ok(())
}

fn read_file(path: &PathBuf) -> Option<CachedSubscription> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(unix)]
fn set_owner_only(f: &std::fs::File) {
    use std::os::unix::fs::PermissionsExt;
    let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_owner_only(_f: &std::fs::File) {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sub(entitled: bool, expires: Option<&str>) -> CachedSubscription {
        CachedSubscription {
            user_id: "user-a".into(),
            entitled,
            tier: "pro".into(),
            seats_total: Some(1),
            seats_claimed: Some(1),
            expires_at: expires.map(|s| s.into()),
            last_checked_at: None,
        }
    }

    fn at(y: i32, mo: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn entitled_and_unexpired_is_entitled() {
        assert!(sub(true, Some("2030-01-01T00:00:00Z")).is_entitled_at(at(2026, 8, 11)));
    }

    #[test]
    fn expired_is_not_entitled() {
        assert!(!sub(true, Some("2025-01-01T00:00:00Z")).is_entitled_at(at(2026, 8, 11)));
    }

    #[test]
    fn not_entitled_stays_not_entitled_before_expiry() {
        // The cancellation case: still inside the window, but the oracle said no.
        assert!(!sub(false, Some("2030-01-01T00:00:00Z")).is_entitled_at(at(2026, 8, 11)));
    }

    #[test]
    fn unparseable_expiry_fails_closed() {
        assert!(!sub(true, Some("not-a-date")).is_entitled_at(at(2026, 8, 11)));
    }

    /// Unlike a perpetual license key, a subscription with no expiry is
    /// malformed rather than eternal — so it must not read as entitled.
    #[test]
    fn missing_expiry_fails_closed() {
        assert!(!sub(true, None).is_entitled_at(at(2026, 8, 11)));
    }

    #[test]
    fn same_user_is_entitled() {
        let s = sub(true, Some("2030-01-01T00:00:00Z"));
        assert!(s.is_entitled_for(Some("user-a"), at(2026, 8, 11)));
    }

    /// The decided sign-out behavior: Pro runs until the cache expires, so no
    /// sequence of clicks can downgrade someone who paid.
    #[test]
    fn signed_out_keeps_entitlement_until_expiry() {
        let s = sub(true, Some("2030-01-01T00:00:00Z"));
        assert!(s.is_entitled_for(None, at(2026, 8, 11)));
    }

    /// The bypass this guards: sign out, sign in with a free account, get Pro.
    #[test]
    fn different_user_never_inherits_entitlement() {
        let s = sub(true, Some("2030-01-01T00:00:00Z"));
        assert!(!s.is_entitled_for(Some("user-b"), at(2026, 8, 11)));
    }

    /// Signing out does not resurrect an already-expired record either.
    #[test]
    fn signed_out_with_expired_cache_is_not_entitled() {
        let s = sub(true, Some("2025-01-01T00:00:00Z"));
        assert!(!s.is_entitled_for(None, at(2026, 8, 11)));
    }
}
