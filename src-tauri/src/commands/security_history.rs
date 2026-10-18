//! Posture scores kept over time, in a store of their own.
//!
//! # Why a third database
//!
//! The app already has two, and the split between them is by lifetime rather
//! than by subject: `settings.db` holds what the user typed and is never
//! pruned; `metrics.db` holds samples that are pruned aggressively — raw ones
//! survive about an hour.
//!
//! Scan history is a third lifetime. It is sparse — one row per scan, not one
//! per second — and its value *rises* with age, because the whole point is
//! seeing where a score was three months ago. Pruned like metrics it would
//! erase the trend it exists to show; parked in settings it would grow without
//! limit alongside data that must never be deleted. So: `security.db`, its own
//! retention, its own connection.
//!
//! # What is not stored
//!
//! Not the findings. A large image carries thousands, and fifty scans across
//! twenty images would be tens of millions of rows answering a question nobody
//! asks. What a user wants is *is this getting better* and *what is new*. The
//! first needs the score. The second needs a comparison against **one**
//! snapshot — the previous scan's vulnerability ids — so exactly one is kept
//! per image, replaced each time.
//!
//! # Comparability
//!
//! A score is only comparable to another computed by the same scanner under the
//! same rule pack. Both travel with every row so a reader can break the series
//! where they change, rather than drawing a line between two different rulers.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One recorded scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRun {
    pub image_ref: String,
    pub image_digest: String,
    pub score: u32,
    /// The full `ScoreBreakdown`, as JSON, so an old row can still be explained
    /// without re-running a scanner that no longer produces the same numbers.
    pub breakdown_json: String,
    pub pack_version: String,
    pub scanner: String,
    pub scanner_version: String,
    pub db_snapshot_date: Option<String>,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub scanned_at: i64,
}

/// How many runs are kept per image digest.
///
/// Fifty scans is months of history for anyone scanning on a normal cadence,
/// and a few kilobytes on disk.
const RUNS_PER_DIGEST: usize = 50;

pub fn db_path() -> PathBuf {
    crate::path_util::app_data_dir().join("security.db")
}

/// Open a connection with this store's pragmas.
///
/// Each caller opens its own, as `metrics_store` does: sharing one connection
/// behind a global mutex would put a history read behind whatever else holds
/// it, and WAL exists so that is unnecessary.
fn open(path: &std::path::Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path).map_err(|e| format!("Cannot open security.db: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| format!("Cannot configure security.db: {e}"))?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS scan_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            image_ref TEXT NOT NULL,
            image_digest TEXT NOT NULL,
            score INTEGER NOT NULL,
            breakdown_json TEXT NOT NULL,
            pack_version TEXT NOT NULL,
            scanner TEXT NOT NULL,
            scanner_version TEXT NOT NULL,
            db_snapshot_date TEXT,
            critical INTEGER NOT NULL,
            high INTEGER NOT NULL,
            medium INTEGER NOT NULL,
            low INTEGER NOT NULL,
            scanned_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_scan_runs_digest ON scan_runs(image_digest, scanned_at);

        -- Exactly one row per image: the vulnerability ids seen last time. This
        -- is what makes 'what is new' answerable without keeping every finding
        -- of every scan.
        CREATE TABLE IF NOT EXISTS finding_snapshot (
            image_digest TEXT PRIMARY KEY,
            image_ref TEXT NOT NULL,
            finding_ids TEXT NOT NULL,
            scanned_at INTEGER NOT NULL
        );

        -- Runtime events read from Falco. Detection is Falco's; this is a copy
        -- kept so the UI has history after a restart, and it is capped by age
        -- because a busy host can produce thousands an hour.
        CREATE TABLE IF NOT EXISTS falco_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts_ms INTEGER NOT NULL,
            priority TEXT NOT NULL,
            rule TEXT NOT NULL,
            output TEXT NOT NULL,
            source TEXT NOT NULL,
            tags TEXT NOT NULL,
            container_id TEXT,
            container_label TEXT,
            image TEXT,
            fields_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_falco_events_ts ON falco_events(ts_ms DESC);
        ",
    )
    .map_err(|e| format!("Cannot create security.db schema: {e}"))
}

/// How long runtime events are kept.
///
/// Falco events are a stream, not a record: their value is in the days after
/// they fire, and an uncapped table on a busy host grows without limit. Seven
/// days is long enough to investigate something noticed on a Monday.
const FALCO_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Hard cap on rows, independent of age.
///
/// Age alone is not enough: one misconfigured rule can emit tens of thousands
/// of events in an hour, all of them younger than the retention window.
const FALCO_MAX_ROWS: i64 = 20_000;

/// Store a batch of events and apply retention. Returns how many were written.
///
/// Takes a batch because the reader polls: one transaction for a poll's worth
/// of events rather than one per event.
pub fn record_falco_events(
    events: &[(super::falco_bridge::FalcoEvent, Option<String>)],
    now_ms: i64,
) -> Result<usize, String> {
    if events.is_empty() {
        return Ok(0);
    }
    let mut conn = open(&db_path())?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("Cannot begin falco event write: {e}"))?;

    for (event, label) in events {
        tx.execute(
            "INSERT INTO falco_events
             (ts_ms, priority, rule, output, source, tags, container_id, container_label, image, fields_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.ts_ms,
                event.priority.as_str(),
                event.rule,
                // Falco's output line is composed from fields of the host it
                // watches, so it can carry paths and environment values.
                crate::redact::redact(&event.output),
                event.source,
                event.tags.join(","),
                event.container_id,
                label,
                event.image,
                crate::redact::redact(&event.fields.to_string()),
            ],
        )
        .map_err(|e| format!("Cannot store falco event: {e}"))?;
    }

    tx.execute(
        "DELETE FROM falco_events WHERE ts_ms < ?1",
        params![now_ms - FALCO_RETENTION_MS],
    )
    .map_err(|e| format!("Cannot apply falco retention: {e}"))?;

    tx.execute(
        "DELETE FROM falco_events WHERE id NOT IN (
             SELECT id FROM falco_events ORDER BY ts_ms DESC, id DESC LIMIT ?1
         )",
        params![FALCO_MAX_ROWS],
    )
    .map_err(|e| format!("Cannot cap falco events: {e}"))?;

    tx.commit()
        .map_err(|e| format!("Cannot commit falco events: {e}"))?;
    Ok(events.len())
}

/// One stored runtime event, as the UI reads it back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFalcoEvent {
    pub id: i64,
    pub ts_ms: i64,
    pub priority: String,
    pub rule: String,
    pub output: String,
    pub source: String,
    pub tags: Vec<String>,
    pub container_id: Option<String>,
    pub container_label: Option<String>,
    pub image: Option<String>,
}

/// Most recent events first.
pub fn falco_events(limit: i64) -> Result<Vec<StoredFalcoEvent>, String> {
    let conn = open(&db_path())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, ts_ms, priority, rule, output, source, tags,
                    container_id, container_label, image
             FROM falco_events ORDER BY ts_ms DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| format!("Cannot read falco events: {e}"))?;

    let rows = stmt
        .query_map(params![limit.clamp(1, 2000)], |row| {
            let tags: String = row.get(6)?;
            Ok(StoredFalcoEvent {
                id: row.get(0)?,
                ts_ms: row.get(1)?,
                priority: row.get(2)?,
                rule: row.get(3)?,
                output: row.get(4)?,
                source: row.get(5)?,
                tags: tags.split(',').filter(|t| !t.is_empty()).map(str::to_string).collect(),
                container_id: row.get(7)?,
                container_label: row.get(8)?,
                image: row.get(9)?,
            })
        })
        .map_err(|e| format!("Cannot read falco events: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Cannot read falco events: {e}"))
}

/// Record a completed audit, then trim this image's history to the cap.
pub fn record_audit(
    audit: &super::security_scan::SecurityAudit,
    now_ms: i64,
) -> Result<(), String> {
    use super::security_scan::Severity;

    let count = |sev: Severity| {
        audit.scan.findings.iter().filter(|f| f.severity == sev).count() as u32
    };
    let breakdown_json = serde_json::to_string(&audit.score)
        .map_err(|e| format!("Cannot encode score breakdown: {e}"))?;

    let conn = open(&db_path())?;
    conn.execute(
        "INSERT INTO scan_runs (image_ref, image_digest, score, breakdown_json, pack_version,
             scanner, scanner_version, db_snapshot_date, critical, high, medium, low, scanned_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            audit.scan.image_ref,
            audit.scan.image_digest,
            audit.score.total,
            breakdown_json,
            audit.score.inputs.pack_version,
            audit.score.inputs.scanner,
            audit.score.inputs.scanner_version,
            audit.score.inputs.db_snapshot_date,
            count(Severity::Critical),
            count(Severity::High),
            count(Severity::Medium),
            count(Severity::Low),
            now_ms,
        ],
    )
    .map_err(|e| format!("Cannot record scan run: {e}"))?;

    trim(&conn, &audit.scan.image_digest)?;
    Ok(())
}

/// Keep the newest `RUNS_PER_DIGEST` runs for one image; drop the rest.
fn trim(conn: &Connection, digest: &str) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM scan_runs WHERE image_digest = ?1 AND id NOT IN (
             SELECT id FROM scan_runs WHERE image_digest = ?1
             ORDER BY scanned_at DESC, id DESC LIMIT ?2
         )",
        params![digest, RUNS_PER_DIGEST as i64],
    )
    .map_err(|e| format!("Cannot trim scan history: {e}"))
}

/// This image's runs, oldest first, so a chart can plot them directly.
pub fn history(image_digest: &str) -> Result<Vec<ScanRun>, String> {
    let conn = open(&db_path())?;
    let mut stmt = conn
        .prepare(
            "SELECT image_ref, image_digest, score, breakdown_json, pack_version, scanner,
                    scanner_version, db_snapshot_date, critical, high, medium, low, scanned_at
             FROM scan_runs WHERE image_digest = ?1 ORDER BY scanned_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let runs = stmt
        .query_map(params![image_digest], row_to_run)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(runs)
}

/// The column order both readers select. One function so a column added to the
/// query cannot be read into the wrong field by only one of them.
fn row_to_run(r: &rusqlite::Row) -> rusqlite::Result<ScanRun> {
    Ok(ScanRun {
        image_ref: r.get(0)?,
        image_digest: r.get(1)?,
        score: r.get(2)?,
        breakdown_json: r.get(3)?,
        pack_version: r.get(4)?,
        scanner: r.get(5)?,
        scanner_version: r.get(6)?,
        db_snapshot_date: r.get(7)?,
        critical: r.get(8)?,
        high: r.get(9)?,
        medium: r.get(10)?,
        low: r.get(11)?,
        scanned_at: r.get(12)?,
    })
}

/// The most recent runs across every image, newest first.
///
/// [`history`] answers "how has *this* image moved", which is what a chart
/// needs. The activity timeline asks the other question — "what happened, in
/// order" — and has no image in hand to ask about.
pub fn recent(limit: i64) -> Result<Vec<ScanRun>, String> {
    let conn = open(&db_path())?;
    let mut stmt = conn
        .prepare(
            "SELECT image_ref, image_digest, score, breakdown_json, pack_version, scanner,
                    scanner_version, db_snapshot_date, critical, high, medium, low, scanned_at
             FROM scan_runs ORDER BY scanned_at DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let runs = stmt
        .query_map(params![limit.clamp(1, 500)], row_to_run)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(runs)
}

/// Vulnerability ids that were not present the last time this image was seen.
///
/// Returns an empty list the first time an image is scanned: everything is new
/// then, and reporting all of it as *newly appeared* would be a false alarm on
/// the very first run.
pub fn diff_and_store_findings(
    image_digest: &str,
    image_ref: &str,
    finding_ids: &[String],
    now_ms: i64,
) -> Result<Vec<String>, String> {
    let conn = open(&db_path())?;

    let previous: Option<String> = conn
        .query_row(
            "SELECT finding_ids FROM finding_snapshot WHERE image_digest = ?1",
            params![image_digest],
            |r| r.get(0),
        )
        .ok();

    let mut sorted: Vec<String> = finding_ids.to_vec();
    sorted.sort();
    sorted.dedup();
    let encoded = sorted.join("\n");

    conn.execute(
        "INSERT INTO finding_snapshot (image_digest, image_ref, finding_ids, scanned_at)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(image_digest) DO UPDATE SET
             image_ref = excluded.image_ref,
             finding_ids = excluded.finding_ids,
             scanned_at = excluded.scanned_at",
        params![image_digest, image_ref, encoded, now_ms],
    )
    .map_err(|e| format!("Cannot store finding snapshot: {e}"))?;

    let Some(previous) = previous else {
        return Ok(vec![]);
    };
    let seen: std::collections::HashSet<&str> = previous.lines().collect();
    Ok(sorted
        .into_iter()
        .filter(|id| !seen.contains(id.as_str()))
        .collect())
}

/// This image's score history.
///
/// Gated: history is the Pro half of the security page. A Free install never
/// writes rows, so this would return an empty list anyway — refusing says why
/// instead of implying nothing ever happened.
#[tauri::command]
pub async fn security_score_history(
    image_digest: String,
) -> Result<Vec<ScanRun>, crate::error::ColimaError> {
    super::security_policy::require_entitled()?;
    crate::helpers::run_blocking(move || history(&image_digest))
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store on a temp path, so tests never touch the user's history.
    fn temp_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        create_schema(&conn).expect("schema");
        conn
    }

    fn insert(conn: &Connection, digest: &str, score: u32, pack: &str, at: i64) {
        conn.execute(
            "INSERT INTO scan_runs (image_ref, image_digest, score, breakdown_json, pack_version,
                 scanner, scanner_version, db_snapshot_date, critical, high, medium, low, scanned_at)
             VALUES ('app:1', ?1, ?2, '{}', ?3, 'trivy', '0.73.0', NULL, 0,0,0,0, ?4)",
            params![digest, score, pack, at],
        )
        .expect("insert");
    }

    #[test]
    fn retention_keeps_the_newest_runs_for_that_image_only() {
        let conn = temp_conn();
        for i in 0..(RUNS_PER_DIGEST as i64 + 10) {
            insert(&conn, "sha256:aaa", 50, "v1", i);
        }
        insert(&conn, "sha256:bbb", 70, "v1", 1);

        trim(&conn, "sha256:aaa").expect("trim");

        let kept: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM scan_runs WHERE image_digest='sha256:aaa'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, RUNS_PER_DIGEST as i64);

        // Another image's history is untouched by one image's trim.
        let other: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM scan_runs WHERE image_digest='sha256:bbb'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(other, 1);

        // The oldest went, not the newest.
        let oldest: i64 = conn
            .query_row(
                "SELECT MIN(scanned_at) FROM scan_runs WHERE image_digest='sha256:aaa'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(oldest, 10);
    }

    #[test]
    fn a_pack_change_is_visible_in_the_rows_themselves() {
        // The chart breaks the series on this; the store's job is only to
        // record which ruler measured each point.
        let conn = temp_conn();
        insert(&conn, "sha256:aaa", 60, "v1", 1);
        insert(&conn, "sha256:aaa", 82, "v2", 2);

        let mut stmt = conn
            .prepare("SELECT pack_version FROM scan_runs ORDER BY scanned_at")
            .unwrap();
        let packs: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(packs, vec!["v1", "v2"]);
    }

    #[test]
    fn the_first_scan_reports_nothing_as_new() {
        let conn = temp_conn();
        // Same logic as the public function, against the in-memory schema.
        let previous: Option<String> = conn
            .query_row(
                "SELECT finding_ids FROM finding_snapshot WHERE image_digest = 'sha256:aaa'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(previous.is_none(), "no snapshot means no comparison to make");
    }

    #[test]
    fn only_ids_absent_last_time_count_as_new() {
        let conn = temp_conn();
        conn.execute(
            "INSERT INTO finding_snapshot VALUES ('sha256:aaa','app:1','CVE-1\nCVE-2', 1)",
            [],
        )
        .unwrap();

        let previous: String = conn
            .query_row(
                "SELECT finding_ids FROM finding_snapshot WHERE image_digest='sha256:aaa'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let seen: std::collections::HashSet<&str> = previous.lines().collect();
        let now = ["CVE-2", "CVE-3", "CVE-4"];
        let new: Vec<&str> = now.iter().copied().filter(|id| !seen.contains(id)).collect();
        assert_eq!(new, vec!["CVE-3", "CVE-4"]);
    }

    #[test]
    fn the_snapshot_holds_one_row_per_image() {
        let conn = temp_conn();
        for ids in ["CVE-1", "CVE-1\nCVE-2"] {
            conn.execute(
                "INSERT INTO finding_snapshot VALUES ('sha256:aaa','app:1',?1,1)
                 ON CONFLICT(image_digest) DO UPDATE SET finding_ids = excluded.finding_ids",
                params![ids],
            )
            .unwrap();
        }
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM finding_snapshot", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "history of findings is not kept, only the last one");
    }
}
