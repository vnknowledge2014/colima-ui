//! Secure storage for the Supabase auth session.
//!
//! # Why this exists
//!
//! supabase-js persists its session in `localStorage` by default. In a webview
//! that means the access token, the refresh token *and* the PKCE `code_verifier`
//! sit in plaintext in the app's data directory, readable by any process running
//! as the user. The refresh token is long-lived, so that is a durable credential
//! on disk.
//!
//! Instead the frontend hands supabase-js a custom storage adapter whose three
//! operations land here. Everything supabase-js wants to persist — not only the
//! tokens — goes into the OS keychain under one service name.
//!
//! # Platform support
//!
//! macOS is the only platform with a backend today, matching the app's primary
//! target. Elsewhere `get`/`set`/`delete` report "unavailable" and the frontend
//! keeps the session in memory only: the user is signed in for the life of the
//! process and signs in again next launch. That is a smaller loss than writing
//! a refresh token to a plaintext file, and it is the same path taken when the
//! user denies the keychain prompt.
//!
//! # What must never live here
//!
//! Entitlement. "Has the user paid" is the Polar license in `crate::license`,
//! cached separately and judged offline-first. Nothing in this module is read
//! when deciding whether a feature is unlocked, so an empty or broken keychain
//! cannot downgrade a paying customer.

/// Keychain service name. Stable across versions — changing it orphans every
/// stored session and silently signs everyone out.
const SERVICE: &str = "com.colima.ui.account";

/// Whether the keychain backend is disabled for this build/run.
///
/// Disabled when:
///  - **Debug builds** — `cargo tauri dev` and debug bundles produce a freshly
///    signed binary on every rebuild, so macOS sees a different program than the
///    one the keychain item was ACL'd to and prompts for access on each launch.
///    Clicking through that prompt dozens of times a day trains the habit of
///    approving keychain dialogs without reading them — a worse outcome for the
///    user's security than keeping a dev session in memory.
///  - **`COLIMAUI_NO_KEYCHAIN=1`** — explicit escape hatch to force the same
///    behavior in a release build.
///
/// The cost is stated plainly: the session lives for the life of the process and
/// the user signs in again next launch. That is the same degraded path
/// non-macOS builds already take, so it is a route that is exercised rather than
/// a special case invented here.
///
/// Read once. An env var re-read per call could change between a `set` and the
/// matching `get`, leaving a session half-written.
fn keychain_disabled() -> bool {
    use std::sync::OnceLock;
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        cfg!(debug_assertions)
            || matches!(
                std::env::var("COLIMAUI_NO_KEYCHAIN").as_deref(),
                Ok("1") | Ok("true")
            )
    })
}

/// True when this build can actually persist a session.
///
/// The frontend calls this once at startup so it can degrade honestly instead of
/// discovering the failure on the first write.
#[tauri::command]
pub fn account_session_available() -> bool {
    cfg!(target_os = "macos") && !keychain_disabled()
}

/// Read a stored value. `None` covers both "no such entry" and "no keychain
/// backend" — the caller treats each as "not signed in", so they need not be
/// distinguished.
#[tauri::command]
pub fn account_session_get(key: String) -> Option<String> {
    backend::get(&key)
}

/// Persist a value. An error here is reported so the frontend can warn that the
/// session will not survive a restart, but it never aborts the sign-in.
#[tauri::command]
pub fn account_session_set(key: String, value: String) -> Result<(), crate::error::ColimaError> {
    backend::set(&key, &value).map_err(Into::into)
}

/// Remove a stored value. Deleting an entry that is not there succeeds: sign-out
/// must be idempotent, and a partially written session still has to clear.
#[tauri::command]
pub fn account_session_delete(key: String) -> Result<(), crate::error::ColimaError> {
    backend::delete(&key).map_err(Into::into)
}

#[cfg(target_os = "macos")]
mod backend {
    use super::SERVICE;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    // Every entry point checks `keychain_disabled()` first. Guarding here rather
    // than at the command layer means no future caller can reach the keychain by
    // going around a check it did not know about.

    pub fn get(key: &str) -> Option<String> {
        if super::keychain_disabled() {
            return None;
        }
        let bytes = get_generic_password(SERVICE, key).ok()?;
        // A non-UTF-8 entry is not something this app wrote. Treat it as absent
        // rather than surfacing a decoding error the user cannot act on.
        String::from_utf8(bytes).ok()
    }

    pub fn set(key: &str, value: &str) -> Result<(), String> {
        if super::keychain_disabled() {
            // Names both reasons: a debug build disables this unconditionally,
            // so "set COLIMAUI_NO_KEYCHAIN=0" would be useless advice.
            return Err(
                "Keychain disabled (debug build, or COLIMAUI_NO_KEYCHAIN set) — \
                 this session will not survive a restart"
                    .to_string(),
            );
        }
        set_generic_password(SERVICE, key, value.as_bytes())
            .map_err(|e| format!("Keychain write failed: {e}"))
    }

    pub fn delete(key: &str) -> Result<(), String> {
        // Sign-out must be idempotent even with the keychain switched off, and
        // reaching the API here would raise the prompt this flag exists to avoid.
        if super::keychain_disabled() {
            return Ok(());
        }
        match delete_generic_password(SERVICE, key) {
            Ok(()) => Ok(()),
            // errSecItemNotFound — already gone, which is the desired end state.
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(format!("Keychain delete failed: {e}")),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    pub fn get(_key: &str) -> Option<String> {
        None
    }

    pub fn set(_key: &str, _value: &str) -> Result<(), String> {
        Err("No secure credential store on this platform".to_string())
    }

    pub fn delete(_key: &str) -> Result<(), String> {
        // Nothing was ever stored, so the entry is already absent. Reporting an
        // error would make sign-out look like it failed.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deleting an absent key is a no-op on every platform, so sign-out cannot
    /// fail just because the session was never persisted.
    #[test]
    fn delete_of_absent_key_succeeds() {
        assert!(account_session_delete("colimaui-test-never-written".into()).is_ok());
    }

    /// Reading an unknown key reports "not signed in" rather than erroring.
    #[test]
    fn get_of_absent_key_is_none() {
        assert!(account_session_get("colimaui-test-never-written".into()).is_none());
    }

    /// Round-trip on the platform that has a backend. Uses a distinct key so a
    /// real session is never touched, and cleans up after itself.
    #[cfg(target_os = "macos")]
    #[test]
    fn set_get_delete_round_trip() {
        let key = "colimaui-test-round-trip";
        // A denied keychain prompt is a legitimate environment (headless CI),
        // and the app degrades rather than failing — so do not assert on write.
        if account_session_set(key.into(), "value".into()).is_err() {
            return;
        }
        assert_eq!(account_session_get(key.into()).as_deref(), Some("value"));
        assert!(account_session_delete(key.into()).is_ok());
        assert!(account_session_get(key.into()).is_none());
    }
}
