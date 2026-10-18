//! Threshold alerts over the metrics stream.
//!
//! A rule is "this metric above this value for this long". Evaluated on the
//! collector's own batches rather than by polling the database: it is the same
//! data, already in memory, and the cost is one pass over (rules × containers)
//! per tick instead of a query loop that has to guess how far back to look.
//!
//! ## Two conditions, not one
//!
//! A threshold alone fires on every spike, and a container that briefly touches
//! 100% CPU during startup is not an incident. The duration requirement is what
//! makes an alert mean "this is still happening", and the cooldown is what stops
//! one long incident becoming forty notifications.
//!
//! ## Where the rules live
//!
//! In `knowledge.db`, beside the app's other settings — **not** in `metrics.db`.
//! That database is rolled up, aged out and truncated to a size cap; a rule the
//! user typed by hand must survive all three. (The plan called this file
//! `settings.db`; this repo keeps app settings in `knowledge.db`, so that is
//! where they go. The requirement was never the filename — it was that
//! retention cannot reach them.)
//!
//! ## Firing is the frontend's job
//!
//! This module records that a rule fired and publishes an event. The operating
//! system notification is sent by the frontend, which already owns that decision
//! (`src/lib/osNotify.ts`: only when unfocused, only terminal outcomes, never
//! runtime error text). Two notification paths would eventually disagree about
//! all three rules.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::commands::metrics_collector::MetricSample;

/// Which number a rule watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertMetric {
    CpuPct,
    MemPct,
    MemBytes,
    /// An image's posture score fell below a policy's minimum. `value` is the
    /// score, `threshold` the minimum the policy asked for.
    SecurityScore,
    /// New vulnerabilities appeared on an image since it was last scanned.
    /// `value` is how many; `threshold` is 0, because one is already too many.
    NewVulnerabilities,
    /// Falco reported a runtime event at or above the alerting priority.
    /// `value` and `threshold` are priority levels, not counts.
    ///
    /// The detection is Falco's, not ours — this metric exists so a runtime
    /// event reaches the user through the same path as every other alert,
    /// rather than through a second notification system they would have to
    /// discover and configure separately.
    RuntimeEvent,
}

impl AlertMetric {
    /// The sample field this metric reads, when it reads one at all.
    ///
    /// Security metrics do not come from the sampling loop — they are produced
    /// by a scan — so they have no value here and are never handed to
    /// `evaluate_batch`.
    fn value(self, s: &MetricSample) -> Option<f64> {
        match self {
            AlertMetric::CpuPct => Some(s.cpu_pct),
            AlertMetric::MemPct => Some(s.mem_pct),
            AlertMetric::MemBytes => Some(s.mem_bytes as f64),
            AlertMetric::SecurityScore
            | AlertMetric::NewVulnerabilities
            | AlertMetric::RuntimeEvent => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AlertMetric::CpuPct => "cpu_pct",
            AlertMetric::MemPct => "mem_pct",
            AlertMetric::MemBytes => "mem_bytes",
            AlertMetric::SecurityScore => "security_score",
            AlertMetric::NewVulnerabilities => "new_vulnerabilities",
            AlertMetric::RuntimeEvent => "runtime_event",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "cpu_pct" => Some(AlertMetric::CpuPct),
            "mem_pct" => Some(AlertMetric::MemPct),
            "mem_bytes" => Some(AlertMetric::MemBytes),
            "security_score" => Some(AlertMetric::SecurityScore),
            "new_vulnerabilities" => Some(AlertMetric::NewVulnerabilities),
            "runtime_event" => Some(AlertMetric::RuntimeEvent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    pub id: i64,
    pub name: String,
    pub metric: AlertMetric,
    pub threshold: f64,
    /// How long the threshold must stay breached before this fires.
    pub duration_secs: i64,
    /// Silence after firing, so one incident is one notification.
    pub cooldown_secs: i64,
    /// Absent means every container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub container_id: String,
    pub container_name: String,
    pub ts: i64,
    /// The value that tripped it, so the log says how bad rather than only that.
    pub value: f64,
    pub threshold: f64,
    pub metric: AlertMetric,
    /// Set for security events, whose subject is an image rather than a
    /// container. Absent for metric events, which have a container instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_ref: Option<String>,
}

pub fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS alert_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            metric TEXT NOT NULL,
            threshold REAL NOT NULL,
            duration_secs INTEGER NOT NULL,
            cooldown_secs INTEGER NOT NULL,
            container_id TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS alert_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id INTEGER NOT NULL,
            rule_name TEXT NOT NULL,
            container_id TEXT NOT NULL,
            container_name TEXT NOT NULL,
            ts INTEGER NOT NULL,
            value REAL NOT NULL,
            threshold REAL NOT NULL,
            metric TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_alert_events_ts ON alert_events(ts);
        ",
    )
    .map_err(|e| format!("Cannot create alert tables: {e}"))?;

    // `image_ref` arrived after the table shipped. Added rather than baked into
    // the CREATE above, because an install that already has `alert_events`
    // never re-runs it — and a column that only new installs get is worse than
    // no column at all. The duplicate-column error on a second run is the
    // expected outcome, not a failure.
    let _ = conn.execute("ALTER TABLE alert_events ADD COLUMN image_ref TEXT", []);
    Ok(())
}

fn db() -> &'static Mutex<Connection> {
    crate::commands::knowledge_bank::get_db()
}

// ===== Rules =====

fn row_to_rule(r: &rusqlite::Row) -> rusqlite::Result<AlertRule> {
    let metric: String = r.get(2)?;
    Ok(AlertRule {
        id: r.get(0)?,
        name: r.get(1)?,
        // A metric this build does not know is reported as CPU rather than
        // failing the whole list — but it can only get there by hand-editing.
        metric: AlertMetric::parse(&metric).unwrap_or(AlertMetric::CpuPct),
        threshold: r.get(3)?,
        duration_secs: r.get(4)?,
        cooldown_secs: r.get(5)?,
        container_id: r.get(6)?,
        enabled: r.get::<_, i64>(7)? != 0,
    })
}

pub fn list_rules() -> Result<Vec<AlertRule>, String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, metric, threshold, duration_secs, cooldown_secs, container_id, enabled
             FROM alert_rules ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rules = stmt
        .query_map([], row_to_rule)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

/// Validate a rule before it is stored.
///
/// A zero duration turns a threshold into a trigger on every spike, and a zero
/// cooldown turns one incident into a notification per tick — both are the
/// behaviours the duration and cooldown exist to prevent, so neither is allowed
/// to be configured away by accident.
fn validate(rule: &AlertRule) -> Result<(), String> {
    if rule.name.trim().is_empty() {
        return Err("A rule needs a name".into());
    }
    if !rule.threshold.is_finite() {
        return Err("Threshold must be a number".into());
    }
    if rule.duration_secs < 5 {
        return Err("Duration must be at least 5 seconds".into());
    }
    if rule.cooldown_secs < 30 {
        return Err("Cooldown must be at least 30 seconds".into());
    }
    Ok(())
}

pub fn upsert_rule(rule: &AlertRule) -> Result<i64, String> {
    validate(rule)?;
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    if rule.id > 0 {
        conn.execute(
            "UPDATE alert_rules SET name=?1, metric=?2, threshold=?3, duration_secs=?4,
             cooldown_secs=?5, container_id=?6, enabled=?7 WHERE id=?8",
            params![
                rule.name,
                rule.metric.as_str(),
                rule.threshold,
                rule.duration_secs,
                rule.cooldown_secs,
                rule.container_id,
                rule.enabled as i64,
                rule.id
            ],
        )
        .map_err(|e| e.to_string())?;
        drop(conn);
        rules_changed();
        return Ok(rule.id);
    }
    conn.execute(
        "INSERT INTO alert_rules (name, metric, threshold, duration_secs, cooldown_secs, container_id, enabled)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            rule.name,
            rule.metric.as_str(),
            rule.threshold,
            rule.duration_secs,
            rule.cooldown_secs,
            rule.container_id,
            rule.enabled as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    drop(conn);
    rules_changed();
    Ok(id)
}

pub fn delete_rule(id: i64) -> Result<(), String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    conn.execute("DELETE FROM alert_rules WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    drop(conn);
    rules_changed();
    // Events are kept. They record something that happened, and deleting the
    // rule does not un-happen it.
    Ok(())
}

pub fn recent_events(limit: i64) -> Result<Vec<AlertEvent>, String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, rule_id, rule_name, container_id, container_name, ts, value, threshold, metric, image_ref
             FROM alert_events ORDER BY ts DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let events = stmt
        .query_map(params![limit.clamp(1, 500)], |r| {
            let metric: String = r.get(8)?;
            Ok(AlertEvent {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                rule_name: r.get(2)?,
                container_id: r.get(3)?,
                container_name: r.get(4)?,
                ts: r.get(5)?,
                value: r.get(6)?,
                threshold: r.get(7)?,
                metric: AlertMetric::parse(&metric).unwrap_or(AlertMetric::CpuPct),
                image_ref: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(events)
}

// ===== Evaluation =====

/// How long a (rule, container) pair has been over its threshold, and when it
/// last fired. In memory only: a breach that spans a restart starts again,
/// which is the honest answer — nothing observed it while the app was down.
#[derive(Debug, Default, Clone)]
pub struct Violation {
    since_ms: Option<i64>,
    last_fired_ms: Option<i64>,
}

/// Per-(rule, container) evaluation state, threaded through `evaluate_batch`.
pub type AlertState = HashMap<(i64, String), Violation>;

static STATE: LazyLock<Mutex<AlertState>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Forget every in-flight breach.
///
/// Called whenever the rule set changes. State is keyed by rule id, so without
/// this an edited rule inherits the time accrued under its *old* threshold —
/// raising a limit would fire immediately, and lowering one would stay silent
/// through the old cooldown. Deleted rules would also keep their entry forever.
///
/// Losing a partial breach is the right trade: the rule the user just changed is
/// a different question, and its clock should start when they asked it.
fn reset_state() {
    if let Ok(mut state) = STATE.lock() {
        state.clear();
    }
}

/// Decide what a batch should fire, without touching the database or the clock.
///
/// Pure so the same logic can be run over stored history for the backtest — the
/// preview and the real thing must not be two implementations that drift.
pub fn evaluate_batch(
    rules: &[AlertRule],
    samples: &[MetricSample],
    state: &mut AlertState,
    now_ms: i64,
) -> Vec<AlertEvent> {
    let mut fired = Vec::new();

    for rule in rules.iter().filter(|r| r.enabled) {
        for sample in samples {
            if let Some(target) = &rule.container_id {
                if target != &sample.container_id {
                    continue;
                }
            }
            // A security rule watches a scan, not a sample. Such a rule cannot
            // fire here at all, and skipping is what keeps it from being
            // evaluated against an unrelated number.
            let Some(value) = rule.metric.value(sample) else {
                continue;
            };
            let key = (rule.id, sample.container_id.clone());
            let entry = state.entry(key).or_default();

            if value <= rule.threshold {
                // Back under: the next breach starts a fresh clock rather than
                // resuming the old one, or a flapping container would qualify
                // for a "sustained" alert it never sustained.
                entry.since_ms = None;
                continue;
            }

            let since = *entry.since_ms.get_or_insert(now_ms);
            if now_ms - since < rule.duration_secs * 1000 {
                continue;
            }
            if let Some(last) = entry.last_fired_ms {
                if now_ms - last < rule.cooldown_secs * 1000 {
                    continue;
                }
            }

            entry.last_fired_ms = Some(now_ms);
            fired.push(AlertEvent {
                id: 0,
                rule_id: rule.id,
                rule_name: rule.name.clone(),
                container_id: sample.container_id.clone(),
                container_name: sample.name.clone(),
                // A threshold alert is about a number, not an image: there is
                // no reference to carry, and inventing one from the container
                // would put a scan verdict on a CPU spike.
                image_ref: None,
                ts: now_ms,
                value,
                threshold: rule.threshold,
                metric: rule.metric,
            });
        }
    }

    fired
}

fn record(event: &AlertEvent) {
    let Ok(conn) = db().lock() else { return };
    if init(&conn).is_err() {
        return;
    }
    let _ = conn.execute(
        "INSERT INTO alert_events (rule_id, rule_name, container_id, container_name, ts, value, threshold, metric, image_ref)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            event.rule_id,
            event.rule_name,
            event.container_id,
            event.container_name,
            event.ts,
            event.value,
            event.threshold,
            event.metric.as_str(),
            event.image_ref
        ],
    );
}

/// Record a security alert and publish it, through the path metric alerts use.
///
/// Deliberately not a second alert pipeline. A user has one place where alerts
/// arrive, one log to read and one set of notification preferences; a security
/// alert that took its own route would be invisible to all three.
///
/// Callers are responsible for grouping: one alert per image per scan, never
/// one per vulnerability. A CVE database refresh can surface hundreds at once,
/// and hundreds of notifications is the same as none.
pub fn emit_security(
    rule_id: i64,
    rule_name: &str,
    image_ref: &str,
    metric: AlertMetric,
    value: f64,
    threshold: f64,
    now_ms: i64,
) {
    let event = AlertEvent {
        id: 0,
        rule_id,
        rule_name: rule_name.to_string(),
        // A scan has no container. Left empty rather than filled with the image
        // name: a reader filtering by container must not match on these.
        container_id: String::new(),
        container_name: String::new(),
        ts: now_ms,
        value,
        threshold,
        metric,
        image_ref: Some(image_ref.to_string()),
    };
    record(&event);
    // `containerId` and `containerName` are published even though a scan has
    // neither. The subscriber composes its notification title from them, and
    // omitting the keys made it read `undefined.slice(...)`, throw, and get
    // swallowed by its own catch — so every security alert was recorded and
    // then silently never shown. Empty strings keep the frame the same shape as
    // a metric alert; `imageRef` is what actually names the subject.
    crate::sse::publish_sse_event(
        "alert.fired",
        &serde_json::json!({
            "ruleId": event.rule_id,
            "ruleName": event.rule_name,
            "containerId": "",
            "containerName": "",
            "imageRef": image_ref,
            "value": value,
            "threshold": threshold,
            "metric": metric.as_str(),
            "ts": now_ms,
        }),
    );
}

/// The rules, cached, because this is read on every collector tick.
///
/// Reading them from SQLite each time took the global settings mutex — shared
/// with everything else that stores a setting — and re-ran the table DDL twice a
/// second on a tokio worker. Invalidated whenever a rule changes, so it cannot
/// go stale in the direction that matters.
static RULE_CACHE: LazyLock<Mutex<Option<Vec<AlertRule>>>> = LazyLock::new(|| Mutex::new(None));

/// One call for both things a rule change invalidates.
fn rules_changed() {
    if let Ok(mut cache) = RULE_CACHE.lock() {
        *cache = None;
    }
    reset_state();
}

fn cached_rules() -> Vec<AlertRule> {
    if let Ok(cache) = RULE_CACHE.lock() {
        if let Some(rules) = cache.as_ref() {
            return rules.clone();
        }
    }
    let rules = list_rules().unwrap_or_default();
    if let Ok(mut cache) = RULE_CACHE.lock() {
        *cache = Some(rules.clone());
    }
    rules
}

/// Evaluate one live batch: the collector's hook.
///
/// Entitlement is checked here, at the moment of action, rather than when the
/// hook was installed — a lapsed subscription must stop firing without a
/// restart. Both that check and the rule list are cached, because this runs on
/// every tick and the tick belongs to the live view.
pub fn on_batch(samples: &[MetricSample], now_ms: i64) {
    if samples.is_empty() {
        return;
    }
    // Cheapest question first: with no rules there is nothing to evaluate and no
    // reason to ask about entitlement at all.
    let rules = cached_rules();
    if rules.is_empty() || !crate::commands::metrics_store::entitled_now_cached() {
        return;
    }
    let Ok(mut state) = STATE.lock() else { return };
    let fired = evaluate_batch(&rules, samples, &mut state, now_ms);
    drop(state);

    for event in &fired {
        record(event);
        // The frontend decides whether this becomes an OS notification; it owns
        // the rules about focus and about what text may leave the app.
        crate::sse::publish_sse_event(
            "alert.fired",
            &serde_json::json!({
                "ruleId": event.rule_id,
                "ruleName": event.rule_name,
                "containerId": event.container_id,
                "containerName": event.container_name,
                "ts": event.ts,
                "value": event.value,
                "threshold": event.threshold,
                "metric": event.metric.as_str(),
            }),
        );
    }
}

// ===== Backtest =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResult {
    /// How many times this rule would have fired over the window.
    pub would_fire: usize,
    /// The window actually covered by stored history, which may be shorter than
    /// the one asked for.
    pub from_ms: i64,
    pub to_ms: i64,
    pub sample_count: usize,
}

/// Replay a rule over stored history.
///
/// This is the only way somebody picks a sensible threshold on the first try:
/// "80% for 5 minutes" means nothing until you know it would have fired eleven
/// times last week.
///
/// The evaluator is the same one the live path uses, so the *logic* cannot
/// drift. The data can: anything older than an hour is stored as per-minute
/// averages, so a rule that keys on brief spikes will backtest lower than it
/// will fire. The UI says this rather than implying the count is exact.
pub fn backtest(rule: &AlertRule, from_ms: i64, to_ms: i64) -> Result<BacktestResult, String> {
    let series = crate::commands::metrics_store::history(from_ms, to_ms, &[])?;
    let mut state = AlertState::new();
    let mut would_fire = 0usize;

    // One point at a time, in order: the evaluator is a state machine over time,
    // and handing it the whole range at once would collapse the duration test.
    for point in &series.points {
        let sample = MetricSample {
            ts: point.ts,
            instance: String::new(),
            container_id: point.container_id.clone(),
            name: series
                .names
                .get(&point.container_id)
                .cloned()
                .unwrap_or_else(|| point.container_id.clone()),
            cpu_pct: point.cpu_pct,
            mem_bytes: point.mem_bytes.max(0) as u64,
            mem_limit_bytes: 0,
            mem_pct: point.mem_pct,
            net_rx_bytes: point.net_rx_bytes.max(0) as u64,
            net_tx_bytes: point.net_tx_bytes.max(0) as u64,
            block_read_bytes: point.block_read_bytes.max(0) as u64,
            block_write_bytes: point.block_write_bytes.max(0) as u64,
            pids: 0,
        };
        would_fire += evaluate_batch(std::slice::from_ref(rule), &[sample], &mut state, point.ts).len();
    }

    Ok(BacktestResult {
        would_fire,
        from_ms: series.points.first().map_or(from_ms, |p| p.ts),
        to_ms: series.points.last().map_or(to_ms, |p| p.ts),
        sample_count: series.points.len(),
    })
}

// ===== Commands =====

/// Refuse the whole alert surface when the install is not entitled.
///
/// Firing was already gated; this closes the rest. A Free install listing and
/// editing rules that can never fire is a feature that looks broken rather than
/// absent — and the gate belongs on both sides of the boundary, because the UI
/// is not an authorization mechanism.
fn require_entitled() -> Result<(), crate::error::ColimaError> {
    if crate::commands::metrics_store::entitled_now() {
        Ok(())
    } else {
        Err(crate::error::ColimaError::validation(
            "Metrics alerts need an active subscription",
        ))
    }
}

#[tauri::command]
pub async fn alerts_list_rules() -> Result<Vec<AlertRule>, crate::error::ColimaError> {
    require_entitled()?;
    crate::helpers::run_blocking(list_rules)
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[tauri::command]
pub async fn alerts_save_rule(rule: AlertRule) -> Result<i64, crate::error::ColimaError> {
    let name = rule.name.clone();
    // The entitlement check sits inside the captured block so a refusal is
    // recorded too: "nothing happened and nobody said why" is the gap this log
    // exists to close.
    let result = async move {
        require_entitled()?;
        crate::helpers::run_blocking(move || upsert_rule(&rule))
            .await
            .map_err(crate::error::ColimaError::validation)
    }
    .await;

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "save",
            "alert_rule",
            &name,
        )
        .outcome_of(&result),
    );

    result
}

#[tauri::command]
pub async fn alerts_delete_rule(id: i64) -> Result<(), crate::error::ColimaError> {
    let result = async move {
        require_entitled()?;
        crate::helpers::run_blocking(move || delete_rule(id))
            .await
            .map_err(crate::error::ColimaError::internal)
    }
    .await;

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "delete",
            "alert_rule",
            &id.to_string(),
        )
        .outcome_of(&result),
    );

    result
}

#[tauri::command]
pub async fn alerts_recent_events(limit: Option<i64>) -> Result<Vec<AlertEvent>, crate::error::ColimaError> {
    require_entitled()?;
    let limit = limit.unwrap_or(50);
    crate::helpers::run_blocking(move || recent_events(limit))
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[tauri::command]
pub async fn alerts_backtest(
    rule: AlertRule,
    from_ms: i64,
    to_ms: i64,
) -> Result<BacktestResult, crate::error::ColimaError> {
    require_entitled()?;
    crate::helpers::run_blocking(move || backtest(&rule, from_ms, to_ms))
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(over: impl FnOnce(&mut AlertRule)) -> AlertRule {
        let mut r = AlertRule {
            id: 1,
            name: "busy".into(),
            metric: AlertMetric::CpuPct,
            threshold: 80.0,
            duration_secs: 60,
            cooldown_secs: 300,
            container_id: None,
            enabled: true,
        };
        over(&mut r);
        r
    }

    fn sample(ts: i64, cpu: f64) -> MetricSample {
        MetricSample {
            ts,
            instance: "colima".into(),
            container_id: "abc".into(),
            name: "web".into(),
            cpu_pct: cpu,
            mem_bytes: 100,
            mem_limit_bytes: 1000,
            mem_pct: 10.0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
            pids: 1,
        }
    }

    /// Feed a series of (offset seconds, cpu) readings and collect what fired.
    fn run(r: &AlertRule, readings: &[(i64, f64)]) -> Vec<AlertEvent> {
        let mut state = HashMap::new();
        let mut fired = Vec::new();
        for (secs, cpu) in readings {
            let ts = secs * 1000;
            fired.extend(evaluate_batch(std::slice::from_ref(r), &[sample(ts, *cpu)], &mut state, ts));
        }
        fired
    }

    #[test]
    fn a_brief_spike_does_not_fire() {
        // Startup CPU is not an incident, which is the whole reason a duration
        // requirement exists.
        let fired = run(&rule(|_| {}), &[(0, 95.0), (10, 95.0), (20, 5.0)]);
        assert!(fired.is_empty());
    }

    #[test]
    fn a_sustained_breach_fires_once() {
        let fired = run(
            &rule(|_| {}),
            &[(0, 95.0), (30, 95.0), (61, 95.0), (90, 95.0), (120, 95.0)],
        );
        assert_eq!(fired.len(), 1, "one incident is one notification");
        assert_eq!(fired[0].container_name, "web");
        assert_eq!(fired[0].value, 95.0);
    }

    #[test]
    fn the_cooldown_expires_and_a_continuing_breach_fires_again() {
        // Still broken five minutes later is worth saying a second time.
        let r = rule(|r| r.cooldown_secs = 300);
        let mut readings: Vec<(i64, f64)> = Vec::new();
        for minute in 0..12 {
            readings.push((minute * 60, 95.0));
        }
        let fired = run(&r, &readings);
        // 60s (duration met), then every cooldown after that: 360s and 660s.
        assert_eq!(fired.len(), 3, "still broken is worth saying again, but not every tick");
        let gaps: Vec<i64> = fired.windows(2).map(|w| w[1].ts - w[0].ts).collect();
        assert!(gaps.iter().all(|g| *g >= 300_000), "cooldown not respected: {gaps:?}");
    }

    #[test]
    fn dropping_below_the_threshold_restarts_the_clock() {
        // A flapping container must not accumulate credit towards "sustained".
        let fired = run(
            &rule(|_| {}),
            &[(0, 95.0), (30, 10.0), (40, 95.0), (70, 95.0), (95, 95.0)],
        );
        assert!(fired.is_empty(), "the breach was never continuous for 60s");
    }

    #[test]
    fn a_rule_scoped_to_one_container_ignores_the_others() {
        let r = rule(|r| r.container_id = Some("other".into()));
        let fired = run(&r, &[(0, 95.0), (61, 95.0), (120, 95.0)]);
        assert!(fired.is_empty());
    }

    #[test]
    fn a_disabled_rule_evaluates_to_nothing() {
        let r = rule(|r| r.enabled = false);
        assert!(run(&r, &[(0, 99.0), (61, 99.0)]).is_empty());
    }

    #[test]
    fn thresholds_that_would_defeat_their_own_purpose_are_refused() {
        assert!(validate(&rule(|r| r.duration_secs = 0)).is_err());
        assert!(validate(&rule(|r| r.cooldown_secs = 0)).is_err());
        assert!(validate(&rule(|r| r.name = "  ".into())).is_err());
        assert!(validate(&rule(|r| r.threshold = f64::NAN)).is_err());
        assert!(validate(&rule(|_| {})).is_ok());
    }

    #[test]
    fn memory_rules_read_the_memory_column() {
        let r = rule(|r| {
            r.metric = AlertMetric::MemPct;
            r.threshold = 5.0;
        });
        // The sample's mem_pct is 10, its cpu is 1 — a rule reading the wrong
        // column would silently never fire.
        let mut state = HashMap::new();
        let mut fired = Vec::new();
        for secs in [0i64, 30, 61] {
            let mut s = sample(secs * 1000, 1.0);
            s.mem_pct = 10.0;
            fired.extend(evaluate_batch(std::slice::from_ref(&r), &[s], &mut state, secs * 1000));
        }
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].metric, AlertMetric::MemPct);
    }
}
