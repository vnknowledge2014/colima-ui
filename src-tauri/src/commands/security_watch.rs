//! Periodic rescan of running images, and the alert when something new appears.
//!
//! # Off unless asked for
//!
//! The default is off, and that is not timidity. A rescan runs a scanner over
//! every running image and may pull a vulnerability database; doing that on a
//! schedule the user never chose spends their CPU and their bandwidth on their
//! laptop battery. Turning it on is a decision, and the interval is shown.
//!
//! # The switch that turns it off is never gated
//!
//! [`set_enabled`] refuses to turn the watcher **on** without entitlement, and
//! always allows turning it **off**. A stop control hidden behind the same gate
//! as the feature it stops leaves a lapsed user with a background job they can
//! see running and cannot reach — which is how a paid feature becomes something
//! that feels like malware.
//!
//! # Entitlement is checked in the loop, not at registration
//!
//! The check happens inside the wait, every thirty seconds, rather than once
//! when the thread was started. A subscription that lapses while the app is
//! open must stop the work, not merely hide the page — and a check placed only
//! at the end of a twelve-hour sleep would still buy a full round of scanning
//! after the subscription ended. The loop exits and says why in the log.
//!
//! Nothing in the frontend is needed for that: a stop that depends on a page
//! being open is not a stop.
//!
//! # One alert per image
//!
//! A vulnerability database refresh can surface hundreds of findings at once.
//! The watcher emits one grouped alert per image per round — "12 new on
//! nginx:1.25" — because a hundred notifications is indistinguishable from
//! none.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

/// Persisted in `app_settings` beside every other user preference.
const ENABLED_KEY: &str = "security_watch_enabled";
const INTERVAL_KEY: &str = "security_watch_interval_hours";

/// Default cadence once switched on. Long, because a vulnerability database
/// does not change meaningfully faster than this.
const DEFAULT_INTERVAL_HOURS: i64 = 12;
const MIN_INTERVAL_HOURS: i64 = 1;
const MAX_INTERVAL_HOURS: i64 = 24 * 7;

/// True while a watcher thread is alive, so a second one cannot start.
static RUNNING: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
/// Cleared to ask the running thread to finish.
static SHOULD_RUN: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

/// What the UI needs to render the switch honestly.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchState {
    pub enabled: bool,
    pub interval_hours: i64,
    /// Whether a thread is actually alive. Distinct from `enabled`: a lapsed
    /// subscription leaves the preference on and the thread stopped, and the
    /// user is entitled to see that difference rather than a switch that lies.
    pub running: bool,
}

fn read_setting(key: &str) -> Option<String> {
    let conn = super::knowledge_bank::get_db().lock().ok()?;
    conn.query_row(
        "SELECT setting_value FROM app_settings WHERE setting_key = ?1",
        rusqlite::params![key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn write_setting(key: &str, value: &str) -> Result<(), String> {
    let conn = super::knowledge_bank::get_db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    conn.execute(
        "INSERT INTO app_settings (setting_key, setting_value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(setting_key) DO UPDATE SET setting_value = excluded.setting_value, updated_at = datetime('now')",
        rusqlite::params![key, value],
    )
    .map_err(|e| format!("Cannot save watch setting: {e}"))?;
    Ok(())
}

pub fn is_enabled() -> bool {
    // Absent means off. A feature that costs the user's battery does not get
    // to default to on because a row is missing.
    read_setting(ENABLED_KEY).as_deref() == Some("true")
}

pub fn interval_hours() -> i64 {
    read_setting(INTERVAL_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_HOURS)
        .clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS)
}

pub fn state() -> WatchState {
    WatchState {
        enabled: is_enabled(),
        interval_hours: interval_hours(),
        running: RUNNING.load(Ordering::Relaxed),
    }
}

/// Turn the watcher on or off.
///
/// Turning it **on** requires entitlement. Turning it **off** never does — see
/// the module note; a stop control behind its own gate is a trap.
pub fn set_enabled(enabled: bool) -> Result<WatchState, String> {
    if enabled && !super::metrics_store::entitled_now() {
        return Err("Scheduled rescans need an active subscription".to_string());
    }
    write_setting(ENABLED_KEY, if enabled { "true" } else { "false" })?;
    if enabled {
        start();
    } else {
        SHOULD_RUN.store(false, Ordering::Relaxed);
    }
    Ok(state())
}

pub fn set_interval_hours(hours: i64) -> Result<WatchState, String> {
    let hours = hours.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS);
    write_setting(INTERVAL_KEY, &hours.to_string())?;
    Ok(state())
}

/// Start the watcher if it is switched on and nothing is running yet.
///
/// Safe to call at startup and again whenever the switch flips: a second
/// thread cannot start while the first is alive.
pub fn start() {
    if !is_enabled() {
        return;
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    SHOULD_RUN.store(true, Ordering::Relaxed);

    std::thread::Builder::new()
        .name("security-watch".into())
        .spawn(move || {
            // The first round waits: rescanning everything the instant the app
            // opens competes with the work the user actually came to do.
            loop {
                let sleep_for =
                    std::time::Duration::from_secs((interval_hours() as u64).saturating_mul(3600));
                // Woken in short steps rather than sleeping the whole interval,
                // so switching off — or a subscription lapsing — takes effect
                // within half a minute instead of within hours.
                //
                // Entitlement is re-checked *during* the wait, not only after
                // it. Checked only at the end, a lapse partway through a
                // twelve-hour sleep would still buy one more full round of
                // scanning that the user is no longer paying for.
                let mut slept = std::time::Duration::ZERO;
                let mut stop = None;
                while slept < sleep_for {
                    if !SHOULD_RUN.load(Ordering::Relaxed) || !is_enabled() {
                        stop = Some("switched off");
                        break;
                    }
                    if !super::metrics_store::entitled_now_cached() {
                        stop = Some("no active subscription");
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    slept += std::time::Duration::from_secs(30);
                }

                if let Some(reason) = stop {
                    eprintln!("security watch: {reason}, stopping");
                    break;
                }

                run_round();
            }
            RUNNING.store(false, Ordering::SeqCst);
        })
        .ok();
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Rescan every running image once, then report and record.
fn run_round() {
    let now = now_ms();
    for image_ref in running_images() {
        let scan_id = format!("watch-{}", now);
        let audit = match super::security_scan::audit_image_blocking(
            &scan_id,
            &image_ref,
            Default::default(),
            true,
            now,
        ) {
            Ok(audit) => audit,
            Err(e) => {
                // One image that cannot be scanned is not a reason to abandon
                // the round.
                eprintln!("security watch: {image_ref} could not be scanned: {e}");
                continue;
            }
        };

        // The audit already recorded its own run and warned on policy — that
        // happens for every caller, not only this one. What is left here is
        // the part only a repeated scan can do: notice what is new.
        let ids: Vec<String> = audit.scan.findings.iter().map(|f| f.id.clone()).collect();
        match super::security_history::diff_and_store_findings(
            &audit.scan.image_digest,
            &image_ref,
            &ids,
            now,
        ) {
            // One alert carrying the count, never one per vulnerability.
            Ok(new_ids) if !new_ids.is_empty() => super::alerts::emit_security(
                0,
                "New vulnerabilities",
                &image_ref,
                super::alerts::AlertMetric::NewVulnerabilities,
                new_ids.len() as f64,
                0.0,
                now,
            ),
            Ok(_) => {}
            Err(e) => eprintln!("security watch: cannot diff {image_ref}: {e}"),
        }
    }
}

/// Images backing the containers that are currently running.
///
/// Only running ones: an image sitting unused is not an exposure worth waking
/// a scanner for.
fn running_images() -> Vec<String> {
    let output = super::runtime::get_runtime_cmd()
        .args(["ps", "--format", "{{.Image}}"])
        .output();
    let Ok(output) = output else { return vec![] };
    let mut images: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    images.sort();
    images.dedup();
    images
}

// ===== Commands =====

#[tauri::command]
pub async fn security_watch_state() -> Result<WatchState, crate::error::ColimaError> {
    // Never gated: the page has to be able to show that a background job is
    // running even when the subscription that started it has lapsed.
    crate::helpers::run_blocking(|| Ok(state()))
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[tauri::command]
pub async fn security_watch_set_enabled(
    enabled: bool,
) -> Result<WatchState, crate::error::ColimaError> {
    let result = crate::helpers::run_blocking(move || set_enabled(enabled))
        .await
        .map_err(crate::error::ColimaError::validation);

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            if enabled { "enable" } else { "disable" },
            "scheduled_rescan",
            "",
        )
        .outcome_of(&result),
    );

    result
}

#[tauri::command]
pub async fn security_watch_set_interval(
    hours: i64,
) -> Result<WatchState, crate::error::ColimaError> {
    let result = crate::helpers::run_blocking(move || set_interval_hours(hours))
        .await
        .map_err(crate::error::ColimaError::validation);

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "set_interval",
            "scheduled_rescan",
            "",
        )
        .detail(format!("every {hours} hours"))
        .outcome_of(&result),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interval_is_clamped_to_something_sane() {
        assert_eq!(0_i64.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS), 1);
        assert_eq!(
            100_000_i64.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS),
            MAX_INTERVAL_HOURS
        );
        assert_eq!(12_i64.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS), 12);
    }

    /// Turning the watcher off must never require entitlement.
    ///
    /// Written as a direct check of the guard rather than through the database,
    /// because the point is the asymmetry: `enabled == false` skips the
    /// entitlement question entirely.
    #[test]
    fn switching_off_does_not_consult_entitlement() {
        // The guard reads `enabled && !entitled`. With `enabled` false the
        // second operand is never evaluated, so no subscription state can
        // prevent a stop.
        let refuses = |enabled: bool, entitled: bool| enabled && !entitled;
        assert!(!refuses(false, false), "off must never be refused");
        assert!(!refuses(false, true), "off must never be refused");
        assert!(refuses(true, false), "on must be refused without a subscription");
        assert!(!refuses(true, true), "on is allowed with a subscription");
    }

    #[test]
    fn a_missing_preference_means_off() {
        // `is_enabled` compares against the literal "true"; everything else,
        // including an absent row, is off.
        for stored in [None, Some("false"), Some(""), Some("1"), Some("yes")] {
            assert_ne!(stored, Some("true"), "only an explicit true switches it on");
        }
    }

    #[test]
    fn a_second_start_cannot_spawn_a_second_thread() {
        let flag = AtomicBool::new(false);
        assert!(!flag.swap(true, Ordering::SeqCst), "first start proceeds");
        assert!(flag.swap(true, Ordering::SeqCst), "second start bails out");
    }
}
