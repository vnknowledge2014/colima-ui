//! Self-healing rules: the few things the app is allowed to do on its own.
//!
//! ## Suggesting is the default, acting is opted into one rule at a time
//!
//! Every rule ships as [`HealMode::Suggest`]. Nothing restarts, stops or
//! reboots anything until a person turns that rule to [`HealMode::Auto`]
//! deliberately. An installer that begins restarting containers because the
//! software decided a health check looked bad is a worse failure than the
//! condition it was trying to repair.
//!
//! ## Two of the five rules have no automatic path at all
//!
//! Pruning images and raising a memory limit are advice by nature: only the
//! person running the machine knows whether a 4 GB layer cache is garbage or
//! next week's build. Those two are not "Auto, disabled by default" — the
//! executor has no branch that can run them, and [`HealAction::auto_capable`]
//! refuses to store the mode. A flag can be flipped by a bad migration; a
//! missing branch cannot.
//!
//! ## Where the quota lives, and why it is not its own table
//!
//! A quota that resets when the app restarts is not a quota — a restart loop
//! that trips the limit only has to outlast the process to get its budget back.
//! So the count is derived from `heal_log`, by counting the actions actually
//! executed inside the window. The log is written before the action is reported
//! as done and is never pruned below the quota window, which makes it a truer
//! record than a counter kept alongside it: a separate table can disagree with
//! the log, and then neither can be trusted.
//!
//! ## Which database
//!
//! `knowledge.db`, not the `settings.db` the phase document names. There is no
//! `settings.db` in this repo — phase 2 found the same thing and put
//! `alert_rules` in `knowledge.db` (see `commands/alerts.rs`). What mattered in
//! that decision was keeping user configuration out of the prunable metrics
//! store, and `knowledge.db` satisfies it; inventing a third file to match a
//! name would split configuration across two places for nothing.

use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::docker_events::ContainerEvent;

/// How far back the quota looks. One hour, matching `max_per_hour`.
const QUOTA_WINDOW_MS: i64 = 60 * 60 * 1000;

/// What a rule watches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealTrigger {
    /// Container reported `unhealthy` and stayed that way.
    Unhealthy,
    /// Container died more than `threshold` times inside the window.
    CrashLoop,
    /// VM disk usage above `threshold` percent.
    DiskFull,
    /// Container was killed for exceeding its memory limit.
    OomKilled,
    /// Colima stopped answering.
    VmUnresponsive,
}

impl HealTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unhealthy => "unhealthy",
            Self::CrashLoop => "crash_loop",
            Self::DiskFull => "disk_full",
            Self::OomKilled => "oom_killed",
            Self::VmUnresponsive => "vm_unresponsive",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "unhealthy" => Self::Unhealthy,
            "crash_loop" => Self::CrashLoop,
            "disk_full" => Self::DiskFull,
            "oom_killed" => Self::OomKilled,
            "vm_unresponsive" => Self::VmUnresponsive,
            _ => return None,
        })
    }
}

/// What a rule does when its trigger fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealAction {
    RestartContainer,
    /// Stop rather than restart: a container that has already died five times
    /// is not going to survive a sixth start, and each attempt costs the user
    /// another burst of load.
    StopContainer,
    RestartVm,
    /// Advisory only — see the module docblock.
    SuggestPrune,
    /// Advisory only — see the module docblock.
    SuggestMemLimit,
}

impl HealAction {
    /// Whether this action may ever run without being asked.
    ///
    /// The executor has no branch that runs the advisory two, so this is the
    /// storage-layer half of the same rule rather than the only guard.
    pub fn auto_capable(self) -> bool {
        matches!(
            self,
            Self::RestartContainer | Self::StopContainer | Self::RestartVm
        )
    }

    /// Public because the activity feed must build the *same* verb string this
    /// writes into `activity_log`; a second spelling means the two rows for one
    /// heal never match and the timeline shows it twice.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RestartContainer => "restart_container",
            Self::StopContainer => "stop_container",
            Self::RestartVm => "restart_vm",
            Self::SuggestPrune => "suggest_prune",
            Self::SuggestMemLimit => "suggest_mem_limit",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "restart_container" => Self::RestartContainer,
            "stop_container" => Self::StopContainer,
            "restart_vm" => Self::RestartVm,
            "suggest_prune" => Self::SuggestPrune,
            "suggest_mem_limit" => Self::SuggestMemLimit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealMode {
    Suggest,
    Auto,
}

impl HealMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Suggest => "suggest",
            Self::Auto => "auto",
        }
    }

    fn parse(s: &str) -> Self {
        // Anything unrecognised falls back to the mode that does nothing.
        // Reading a corrupted row as `Auto` would act on a value nobody wrote.
        if s == "auto" {
            Self::Auto
        } else {
            Self::Suggest
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealRule {
    pub id: i64,
    pub name: String,
    pub trigger: HealTrigger,
    pub action: HealAction,
    pub mode: HealMode,
    /// Threshold whose meaning depends on the trigger: minutes unhealthy,
    /// deaths per window, percent of disk.
    pub threshold: f64,
    /// Window for `CrashLoop`, in seconds. Ignored by the other triggers.
    pub window_secs: i64,
    pub max_per_hour: i64,
    pub enabled: bool,
    /// Mirrors [`HealAction::auto_capable`] so the UI can grey out the toggle
    /// instead of offering a switch the backend will refuse.
    pub auto_capable: bool,
}

/// What became of a firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealOutcome {
    /// Acted, and the action reported success.
    Executed,
    /// Acted, and the action failed.
    Failed,
    /// Recorded as advice; nothing was done to the machine.
    Suggested,
    /// Would have acted, but the hourly quota was already spent.
    QuotaBlocked,
    /// Would have acted, but the kill switch was off.
    SwitchedOff,
}

impl HealOutcome {
    /// Every variant, so the quota query can ask which ones spend budget
    /// instead of repeating the list in SQL.
    const ALL: [Self; 5] = [
        Self::Executed,
        Self::Failed,
        Self::Suggested,
        Self::QuotaBlocked,
        Self::SwitchedOff,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::Executed => "executed",
            Self::Failed => "failed",
            Self::Suggested => "suggested",
            Self::QuotaBlocked => "quota_blocked",
            Self::SwitchedOff => "switched_off",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "executed" => Self::Executed,
            "failed" => Self::Failed,
            "quota_blocked" => Self::QuotaBlocked,
            "switched_off" => Self::SwitchedOff,
            _ => Self::Suggested,
        }
    }

    /// Whether this outcome spent a unit of quota.
    ///
    /// Only actions that reached the machine count. A firing blocked by the
    /// quota itself must not consume quota, or one blocked attempt would
    /// extend the block for another hour.
    fn spends_quota(self) -> bool {
        matches!(self, Self::Executed | Self::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealLogEntry {
    pub id: i64,
    pub ts: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub container_id: String,
    pub container_name: String,
    pub action: HealAction,
    pub mode: HealMode,
    pub outcome: HealOutcome,
    /// Human-readable specifics: the real numbers behind a suggestion, or the
    /// error behind a failure. Never empty — an unexplained log line is the
    /// silent action this phase exists to prevent.
    pub detail: String,
}

fn db() -> &'static Mutex<Connection> {
    crate::commands::knowledge_bank::get_db()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS heal_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            trigger_kind TEXT NOT NULL UNIQUE,
            action TEXT NOT NULL,
            mode TEXT NOT NULL DEFAULT 'suggest',
            threshold REAL NOT NULL,
            window_secs INTEGER NOT NULL DEFAULT 0,
            max_per_hour INTEGER NOT NULL DEFAULT 3,
            enabled INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS heal_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            rule_id INTEGER NOT NULL,
            rule_name TEXT NOT NULL,
            container_id TEXT NOT NULL DEFAULT '',
            container_name TEXT NOT NULL DEFAULT '',
            action TEXT NOT NULL,
            mode TEXT NOT NULL,
            outcome TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_heal_log_rule_ts ON heal_log(rule_id, ts);
        CREATE TABLE IF NOT EXISTS heal_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            enabled INTEGER NOT NULL DEFAULT 1
        );
        INSERT OR IGNORE INTO heal_config (id, enabled) VALUES (1, 1);
        ",
    )
    .map_err(|e| format!("Cannot create self-healing tables: {e}"))?;
    seed(conn)
}

/// The five rules, every one of them advisory on a fresh install.
///
/// `INSERT OR IGNORE` against the unique trigger: seeding must never reopen a
/// rule the user has since turned off, and must never quietly move one they
/// switched to Auto back to Suggest.
fn seed(conn: &Connection) -> Result<(), String> {
    const SEED: &[(&str, HealTrigger, HealAction, f64, i64, i64)] = &[
        (
            "Restart a container that stays unhealthy",
            HealTrigger::Unhealthy,
            HealAction::RestartContainer,
            5.0,
            0,
            3,
        ),
        (
            "Stop a container that is crash-looping",
            HealTrigger::CrashLoop,
            HealAction::StopContainer,
            5.0,
            120,
            3,
        ),
        (
            "Suggest a prune when the VM disk fills",
            HealTrigger::DiskFull,
            HealAction::SuggestPrune,
            85.0,
            0,
            2,
        ),
        (
            "Suggest a higher memory limit after an OOM kill",
            HealTrigger::OomKilled,
            HealAction::SuggestMemLimit,
            1.0,
            0,
            5,
        ),
        (
            "Restart Colima when it stops answering",
            HealTrigger::VmUnresponsive,
            HealAction::RestartVm,
            2.0,
            0,
            1,
        ),
    ];

    for (name, trigger, action, threshold, window, quota) in SEED {
        conn.execute(
            "INSERT OR IGNORE INTO heal_rules
             (name, trigger_kind, action, mode, threshold, window_secs, max_per_hour, enabled)
             VALUES (?1, ?2, ?3, 'suggest', ?4, ?5, ?6, 1)",
            rusqlite::params![
                name,
                trigger.as_str(),
                action.as_str(),
                threshold,
                window,
                quota
            ],
        )
        .map_err(|e| format!("Cannot seed self-healing rule: {e}"))?;
    }
    Ok(())
}

fn row_to_rule(r: &rusqlite::Row) -> rusqlite::Result<HealRule> {
    let trigger: String = r.get(2)?;
    let action: String = r.get(3)?;
    let mode: String = r.get(4)?;
    let action = HealAction::parse(&action).unwrap_or(HealAction::SuggestPrune);
    let stored_mode = HealMode::parse(&mode);
    Ok(HealRule {
        id: r.get(0)?,
        name: r.get(1)?,
        trigger: HealTrigger::parse(&trigger).unwrap_or(HealTrigger::Unhealthy),
        action,
        // An advisory action read back as Auto is a corrupted row, not a
        // permission. Downgrade on read as well as refusing on write.
        mode: if action.auto_capable() {
            stored_mode
        } else {
            HealMode::Suggest
        },
        threshold: r.get(5)?,
        window_secs: r.get(6)?,
        max_per_hour: r.get(7)?,
        enabled: r.get::<_, i64>(8)? != 0,
        auto_capable: action.auto_capable(),
    })
}

pub fn list_rules() -> Result<Vec<HealRule>, String> {
    let conn = db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, trigger_kind, action, mode, threshold, window_secs, max_per_hour, enabled
             FROM heal_rules ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rules = stmt
        .query_map([], row_to_rule)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

/// Change the parts of a rule a user is allowed to change.
///
/// Trigger and action are not among them: they define which rule this is, and
/// letting the UI rewrite them would turn "restart unhealthy containers" into
/// "reboot the VM" behind a name that still says otherwise.
pub fn update_rule(
    id: i64,
    mode: HealMode,
    threshold: f64,
    max_per_hour: i64,
    enabled: bool,
) -> Result<(), String> {
    if !threshold.is_finite() || threshold <= 0.0 {
        return Err("Threshold must be a positive number".into());
    }
    if max_per_hour < 1 {
        return Err("A rule that may act must be allowed at least one action per hour".into());
    }

    let conn = db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;

    let action: String = conn
        .query_row(
            "SELECT action FROM heal_rules WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| "No such rule".to_string())?;
    let action = HealAction::parse(&action).ok_or_else(|| "Unknown action".to_string())?;
    if mode == HealMode::Auto && !action.auto_capable() {
        return Err("This rule can only ever suggest — it has no automatic action".into());
    }

    conn.execute(
        "UPDATE heal_rules SET mode=?1, threshold=?2, max_per_hour=?3, enabled=?4 WHERE id=?5",
        rusqlite::params![mode.as_str(), threshold, max_per_hour, enabled as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Kill switch =====

/// Whether self-healing may act at all.
///
/// Read at the moment of acting, so flipping it stops work already queued
/// rather than only preventing new work.
pub fn is_enabled() -> bool {
    let Ok(conn) = db().lock() else {
        return false;
    };
    if init(&conn).is_err() {
        return false;
    }
    conn.query_row("SELECT enabled FROM heal_config WHERE id = 1", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|v| v != 0)
    // Unreadable configuration means "do not act": the failure mode of a
    // silent no-op is a container left down, and the failure mode of the
    // opposite is a machine being restarted by software that cannot read
    // its own settings.
    .unwrap_or(false)
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let conn = db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    conn.execute(
        "UPDATE heal_config SET enabled = ?1 WHERE id = 1",
        [on as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Log and quota =====

fn record(entry: &HealLogEntry) -> Result<(), String> {
    let conn = db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    conn.execute(
        "INSERT INTO heal_log
         (ts, rule_id, rule_name, container_id, container_name, action, mode, outcome, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            entry.ts,
            entry.rule_id,
            entry.rule_name,
            entry.container_id,
            entry.container_name,
            entry.action.as_str(),
            entry.mode.as_str(),
            entry.outcome.as_str(),
            entry.detail,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn recent_log(limit: i64) -> Result<Vec<HealLogEntry>, String> {
    let conn = db()
        .lock()
        .map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, ts, rule_id, rule_name, container_id, container_name, action, mode, outcome, detail
             FROM heal_log ORDER BY ts DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit.clamp(1, 500)], |r| {
            let action: String = r.get(6)?;
            let mode: String = r.get(7)?;
            let outcome: String = r.get(8)?;
            Ok(HealLogEntry {
                id: r.get(0)?,
                ts: r.get(1)?,
                rule_id: r.get(2)?,
                rule_name: r.get(3)?,
                container_id: r.get(4)?,
                container_name: r.get(5)?,
                action: HealAction::parse(&action).unwrap_or(HealAction::SuggestPrune),
                mode: HealMode::parse(&mode),
                outcome: HealOutcome::parse(&outcome),
                detail: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// How many times this rule has reached the machine in the last hour.
///
/// Counted from the log rather than a counter, so it survives a restart of the
/// app without any save step — see the module docblock.
fn spent_this_hour(conn: &Connection, rule_id: i64, now: i64) -> i64 {
    // Built from `spends_quota` rather than naming the outcomes again here:
    // two lists of "what counts against the budget" would eventually disagree,
    // and the one in SQL is the one nobody reads.
    let counted: Vec<&str> = HealOutcome::ALL
        .iter()
        .filter(|o| o.spends_quota())
        .map(|o| o.as_str())
        .collect();
    let placeholders = vec!["?"; counted.len()].join(", ");
    let sql = format!(
        "SELECT COUNT(*) FROM heal_log
         WHERE rule_id = ?1 AND ts >= ?2 AND outcome IN ({placeholders})"
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(rule_id),
        Box::new(now - QUOTA_WINDOW_MS),
    ];
    for o in counted {
        params.push(Box::new(o.to_string()));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    conn.query_row(&sql, refs.as_slice(), |r| r.get::<_, i64>(0))
        .unwrap_or(i64::MAX) // Unreadable quota blocks rather than permits.
}

// ===== Executor =====

/// One firing of one rule, before it is decided what to do about it.
pub struct Firing {
    pub rule: HealRule,
    pub container_id: String,
    pub container_name: String,
    /// The specifics that justify this firing, in the user's words.
    pub detail: String,
}

/// Runs a firing through the same path whether it acts or only advises.
///
/// The gates are ordered by what they protect: the kill switch first, because a
/// person asking it to stop outranks everything; quota last, because it only
/// limits how often something already permitted may happen.
pub async fn execute(firing: Firing) -> HealOutcome {
    let rule = &firing.rule;

    let outcome = if !is_enabled() {
        HealOutcome::SwitchedOff
    } else if rule.mode == HealMode::Suggest || !rule.action.auto_capable() {
        // The advisory path. Note there is no `else` below that can run an
        // advisory action: `SuggestPrune` and `SuggestMemLimit` end here.
        HealOutcome::Suggested
    } else if quota_spent(rule) {
        HealOutcome::QuotaBlocked
    } else {
        match perform(rule.action, &firing.container_id).await {
            Ok(()) => HealOutcome::Executed,
            Err(_) => HealOutcome::Failed,
        }
    };

    let detail = match outcome {
        HealOutcome::QuotaBlocked => format!(
            "{} — blocked, already acted {} times this hour",
            firing.detail, rule.max_per_hour
        ),
        HealOutcome::SwitchedOff => format!("{} — not run, self-healing is off", firing.detail),
        _ => firing.detail.clone(),
    };

    let entry = HealLogEntry {
        id: 0,
        ts: now_ms(),
        rule_id: rule.id,
        rule_name: rule.name.clone(),
        container_id: firing.container_id.clone(),
        container_name: firing.container_name.clone(),
        action: rule.action,
        mode: rule.mode,
        outcome,
        detail,
    };
    // A failure to log is not a reason to hide the action, but there is
    // nowhere left to report it to.
    let _ = record(&entry);
    outcome
}

fn quota_spent(rule: &HealRule) -> bool {
    let Ok(conn) = db().lock() else {
        return true;
    };
    if init(&conn).is_err() {
        return true;
    }
    spent_this_hour(&conn, rule.id, now_ms()) >= rule.max_per_hour
}

/// The only place an automatic action touches the machine.
async fn perform(action: HealAction, container_id: &str) -> Result<(), String> {
    let started = std::time::Instant::now();
    let result = perform_inner(action, container_id).await;

    // Recorded as `app`, so the activity log can answer "did I restart this or
    // did it restart itself" — the question `heal_log` cannot, because it only
    // contains things the app did.
    //
    // No double entry: this path calls the service layer directly, not the
    // Tauri commands that carry their own recording.
    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Lifecycle,
            action.as_str(),
            "container",
            container_id,
        )
        .by(crate::commands::activity::ActivityActor::App)
        .took(started.elapsed().as_millis() as i64)
        .outcome_of(&result),
    );

    result
}

async fn perform_inner(action: HealAction, container_id: &str) -> Result<(), String> {
    let svc = crate::services::container::ContainerService::auto_detect();
    match action {
        HealAction::RestartContainer => svc
            .restart_container(container_id)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        HealAction::StopContainer => svc
            .stop_container(container_id)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        HealAction::RestartVm => crate::commands::colima::start_instance_cli(String::new())
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        // Unreachable by construction: `execute` returns `Suggested` for these
        // before it can get here. Kept as an explicit refusal rather than a
        // catch-all so adding an action cannot silently fall into "do nothing".
        HealAction::SuggestPrune | HealAction::SuggestMemLimit => {
            Err("This action is advisory and is never performed".into())
        }
    }
}

// ===== Triggers =====

/// Per-container state the counting rules need.
///
/// In memory on purpose: this is the detection window, not the quota. Losing it
/// on restart costs at most one missed crash-loop, whereas losing the quota
/// would hand back a spent budget.
#[derive(Default)]
struct Watch {
    /// Death timestamps inside the crash-loop window.
    deaths: HashMap<String, Vec<i64>>,
    /// When each container was first seen unhealthy, and its name.
    unhealthy_since: HashMap<String, (i64, String)>,
}

fn watch() -> &'static Mutex<Watch> {
    static WATCH: std::sync::OnceLock<Mutex<Watch>> = std::sync::OnceLock::new();
    WATCH.get_or_init(|| Mutex::new(Watch::default()))
}

fn rule_for(trigger: HealTrigger) -> Option<HealRule> {
    list_rules()
        .ok()?
        .into_iter()
        .find(|r| r.trigger == trigger && r.enabled)
}

/// Count a death and report whether it completes a crash loop.
///
/// Split from the event loop so the window arithmetic can be tested without a
/// Docker daemon.
fn note_death(id: &str, now: i64, window_ms: i64, threshold: usize) -> bool {
    let Ok(mut w) = watch().lock() else {
        return false;
    };
    let deaths = w.deaths.entry(id.to_string()).or_default();
    deaths.retain(|t| now - *t <= window_ms);
    deaths.push(now);
    let tripped = deaths.len() >= threshold;
    if tripped {
        // Clearing on trip stops one long loop from firing on every subsequent
        // death; the next firing needs a fresh window's worth of evidence.
        deaths.clear();
    }
    tripped
}

/// Subscribe to container events and drive the event-triggered rules.
pub fn spawn_watcher() {
    tauri::async_runtime::spawn(async move {
        let mut rx = crate::docker_events::subscribe();
        loop {
            let event = match rx.recv().await {
                Ok(e) => e,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[SelfHeal] dropped {n} events while behind");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            handle_event(event).await;
        }
    });
}

async fn handle_event(event: ContainerEvent) {
    let now = now_ms();

    if event.is_oom() {
        if let Some(rule) = rule_for(HealTrigger::OomKilled) {
            let detail = suggest_mem_limit_detail(&event.container_id);
            execute(Firing {
                rule,
                container_id: event.container_id.clone(),
                container_name: event.container_name.clone(),
                detail,
            })
            .await;
        }
    }

    if event.action == "die" {
        if let Some(rule) = rule_for(HealTrigger::CrashLoop) {
            let window_ms = rule.window_secs.max(1) * 1000;
            let threshold = rule.threshold.max(1.0) as usize;
            if note_death(&event.container_id, now, window_ms, threshold) {
                execute(Firing {
                    rule,
                    container_id: event.container_id.clone(),
                    container_name: event.container_name.clone(),
                    detail: format!(
                        "died {} times in {} seconds",
                        threshold,
                        window_ms / 1000
                    ),
                })
                .await;
            }
        }
    }

    if event.action.starts_with("health_status") {
        let unhealthy = event.action.contains("unhealthy");
        let Ok(mut w) = watch().lock() else { return };
        if unhealthy {
            w.unhealthy_since
                .entry(event.container_id.clone())
                .or_insert((now, event.container_name.clone()));
        } else {
            w.unhealthy_since.remove(&event.container_id);
        }
    }
}

/// Fire the unhealthy rule for anything that has been unhealthy long enough.
///
/// Time-based rather than event-based: Docker reports the transition once, and
/// "still unhealthy five minutes later" is not an event anybody sends.
pub async fn sweep_unhealthy() {
    let Some(rule) = rule_for(HealTrigger::Unhealthy) else {
        return;
    };
    let cutoff_ms = (rule.threshold.max(1.0) * 60_000.0) as i64;
    let now = now_ms();

    let due: Vec<(String, String)> = {
        let Ok(mut w) = watch().lock() else { return };
        let due: Vec<(String, String)> = w
            .unhealthy_since
            .iter()
            .filter(|(_, (since, _))| now - *since >= cutoff_ms)
            .map(|(id, (_, name))| (id.clone(), name.clone()))
            .collect();
        // Drop them so the rule fires once per unhealthy spell, not once per
        // sweep for as long as the container stays down.
        for (id, _) in &due {
            w.unhealthy_since.remove(id);
        }
        due
    };

    for (id, name) in due {
        execute(Firing {
            rule: rule.clone(),
            container_id: id,
            container_name: name,
            detail: format!("unhealthy for more than {} minutes", rule.threshold as i64),
        })
        .await;
    }
}

/// What can be said about a memory-limit suggestion without recorded history.
///
/// Sizing a limit takes a peak to multiply, and peaks come from stored metric
/// samples, which this build does not keep. Saying so beats inventing a number:
/// a suggestion the user cannot check is a suggestion they should not follow.
fn suggest_mem_limit_detail(_container_id: &str) -> String {
    "killed for using too much memory; no recorded history to size a new limit from".into()
}

/// Poll the conditions that no event reports.
///
/// Docker announces a health transition once and says nothing more; disk usage
/// is never an event at all. Both are therefore swept on a timer. Sixty seconds
/// is chosen against the cheapest rule threshold — a five-minute unhealthy rule
/// cannot be late by more than a minute — and `docker system df` is the only
/// expensive call here, so it runs at a fifth of that rate.
pub fn spawn_sweeper() {
    tauri::async_runtime::spawn(async move {
        let mut ticks: u64 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            ticks += 1;
            sweep_unhealthy().await;
            if ticks.is_multiple_of(5) {
                sweep_disk().await;
            }
        }
    });
}

/// Advise a prune when the VM disk crosses the rule's threshold.
///
/// The numbers come from `docker system df` at the moment of advising, not from
/// a cached figure: telling somebody to reclaim 4 GB that another process
/// already reclaimed wastes the one action they were asked to take.
async fn sweep_disk() {
    let Some(rule) = rule_for(HealTrigger::DiskFull) else {
        return;
    };
    let svc = crate::services::container::ContainerService::auto_detect();
    let Ok(df) = svc.system_df().await else { return };

    let Some(reclaimable) = parse_reclaimable_bytes(&df) else {
        return;
    };
    // `docker system df` reports what can be freed, not how full the disk is,
    // so the threshold is read as "at least this many gigabytes are wasted"
    // when the percentage is not available from the same call.
    let gb = reclaimable as f64 / 1_073_741_824.0;
    if gb < rule.threshold / 10.0 {
        return;
    }

    execute(Firing {
        rule,
        container_id: String::new(),
        container_name: String::new(),
        detail: format!("{gb:.1} GB of images and build cache can be reclaimed"),
    })
    .await;
}

/// Total reclaimable bytes across every section of `docker system df`.
///
/// Parsed rather than taken from `--format json` because the adapter already
/// returns the human table, and the sizes there carry their own units.
fn parse_reclaimable_bytes(output: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut seen = false;
    for line in output.lines().skip(1) {
        let Some(last) = line.split_whitespace().last() else {
            continue;
        };
        // The reclaimable column is either `1.2GB` or `1.2GB (50%)`; the split
        // above lands on the percentage in the second case.
        let field = if last.ends_with(')') {
            line.split_whitespace().rev().nth(1)?
        } else {
            last
        };
        if let Some(bytes) = parse_size(field) {
            total += bytes;
            seen = true;
        }
    }
    seen.then_some(total)
}

fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = s.split_at(split);
    let num: f64 = num.parse().ok()?;
    let mult: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "tb" => 1_000_000_000_000.0,
        _ => return None,
    };
    Some((num * mult) as i64)
}

// ===== Commands =====

#[tauri::command]
pub async fn self_heal_list_rules() -> Result<Vec<HealRule>, crate::error::ColimaError> {
    list_rules().map_err(crate::error::ColimaError::from)
}

#[tauri::command]
pub async fn self_heal_save_rule(
    id: i64,
    mode: HealMode,
    threshold: f64,
    max_per_hour: i64,
    enabled: bool,
) -> Result<(), crate::error::ColimaError> {
    let result =
        update_rule(id, mode, threshold, max_per_hour, enabled).map_err(crate::error::ColimaError::from);

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "save",
            "heal_rule",
            &id.to_string(),
        )
        .detail(format!("mode {}, at most {max_per_hour}/hour, enabled {enabled}", mode.as_str()))
        .outcome_of(&result),
    );

    result
}

#[tauri::command]
pub async fn self_heal_recent_log(
    limit: Option<i64>,
) -> Result<Vec<HealLogEntry>, crate::error::ColimaError> {
    recent_log(limit.unwrap_or(50)).map_err(crate::error::ColimaError::from)
}

/// Whether self-healing may act.
#[tauri::command]
pub async fn self_heal_is_enabled() -> bool {
    is_enabled()
}

#[tauri::command]
pub async fn self_heal_set_enabled(on: bool) -> Result<(), crate::error::ColimaError> {
    let result = set_enabled(on).map_err(crate::error::ColimaError::from);

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            if on { "enable" } else { "disable" },
            "self_healing",
            "",
        )
        .outcome_of(&result),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_actions_are_never_auto_capable() {
        assert!(!HealAction::SuggestPrune.auto_capable());
        assert!(!HealAction::SuggestMemLimit.auto_capable());
        assert!(HealAction::RestartContainer.auto_capable());
        assert!(HealAction::StopContainer.auto_capable());
        assert!(HealAction::RestartVm.auto_capable());
    }

    #[test]
    fn an_unreadable_mode_reads_as_suggest() {
        assert_eq!(HealMode::parse("auto"), HealMode::Auto);
        assert_eq!(HealMode::parse("suggest"), HealMode::Suggest);
        assert_eq!(HealMode::parse(""), HealMode::Suggest);
        assert_eq!(HealMode::parse("AUTO"), HealMode::Suggest);
    }

    #[test]
    fn only_actions_that_reached_the_machine_spend_quota() {
        assert!(HealOutcome::Executed.spends_quota());
        assert!(HealOutcome::Failed.spends_quota());
        assert!(!HealOutcome::QuotaBlocked.spends_quota());
        assert!(!HealOutcome::Suggested.spends_quota());
        assert!(!HealOutcome::SwitchedOff.spends_quota());
    }

    #[test]
    fn a_crash_loop_needs_its_deaths_inside_the_window() {
        let id = "crash-window";
        // Four deaths spread over four minutes never trip a 5-in-2-minutes rule.
        for i in 0..4 {
            assert!(!note_death(id, i * 60_000, 120_000, 5));
        }
    }

    #[test]
    fn five_deaths_inside_the_window_trip_the_rule() {
        let id = "crash-burst";
        for i in 0..4 {
            assert!(!note_death(id, 1_000_000 + i * 1_000, 120_000, 5));
        }
        assert!(note_death(id, 1_000_000 + 5_000, 120_000, 5));
    }

    #[test]
    fn tripping_resets_the_window() {
        let id = "crash-reset";
        for i in 0..4 {
            assert!(!note_death(id, 2_000_000 + i * 1_000, 120_000, 5));
        }
        assert!(note_death(id, 2_000_000 + 5_000, 120_000, 5));
        // The very next death must not fire again on the strength of the old
        // burst — otherwise one loop reports forever.
        assert!(!note_death(id, 2_000_000 + 6_000, 120_000, 5));
    }

    #[test]
    fn round_trips_every_trigger_and_action_name() {
        for t in [
            HealTrigger::Unhealthy,
            HealTrigger::CrashLoop,
            HealTrigger::DiskFull,
            HealTrigger::OomKilled,
            HealTrigger::VmUnresponsive,
        ] {
            assert_eq!(HealTrigger::parse(t.as_str()), Some(t));
        }
        for a in [
            HealAction::RestartContainer,
            HealAction::StopContainer,
            HealAction::RestartVm,
            HealAction::SuggestPrune,
            HealAction::SuggestMemLimit,
        ] {
            assert_eq!(HealAction::parse(a.as_str()), Some(a));
        }
    }
}
