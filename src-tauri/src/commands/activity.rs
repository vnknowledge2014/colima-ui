//! What the user did to this machine, kept locally.
//!
//! The app already records the **results** of four things — self-healing, alerts,
//! scans, compose patches. It records no **actions**: who pressed prune, when,
//! and what went with it. `prune_images` cannot be undone and until now left no
//! trace at all beyond a changed disk figure.
//!
//! ## Recording never breaks the action
//!
//! [`record`] returns nothing and cannot fail upward. Its caller is in the
//! middle of `remove_container`; there is nothing useful to do with a logging
//! error there, and the only way to misuse one is to let it escape and turn a
//! successful removal into a failed one. Errors go to `eprintln!`, following
//! `security_scan.rs` when `record_audit` fails.
//!
//! ## Failures are recorded too
//!
//! A refused prune is worth knowing about. `outcome` is a required column, not
//! something written only on success.
//!
//! ## No raw argv
//!
//! There is no `raw_command` column, because a full argv invites storing
//! `-e PASSWORD=…`. `detail` is a short sentence composed by the caller, and
//! [`redact`](crate::redact::redact) is applied **here** rather than trusted to
//! the caller — this database travels inside the diagnostic bundle a user
//! sends when asking for help.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// What the log keeps: a week, or five hundred actions, whichever comes first.
///
/// A store with no upper bound is a slow disk leak nobody notices until it
/// matters, and a week is far longer than the window anyone reads back.
const RETENTION_DAYS: i64 = 7;
const MAX_ROWS: i64 = 500;

/// Retention runs on this many writes rather than on a timer.
///
/// This is sparse data — a background thread to prune it would be machinery
/// bought for nothing.
const RETENTION_EVERY: u64 = 50;

static WRITES: AtomicU64 = AtomicU64::new(0);

/// Which question this row answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// Cannot be undone: prune, remove, delete.
    Destructive,
    /// Start, stop, restart — the shape of "why did it die last night".
    Lifecycle,
    /// Took time and either finished or did not: pull, save, load, copy.
    Task,
    /// Changed how the machine behaves afterwards.
    Config,
}

impl ActivityKind {
    /// The wire spelling — the same one the column stores and the frontend
    /// sends. Public so no caller is tempted to derive it from `Debug`:
    /// `Debug` drops the underscore a multi-word variant would need, and a
    /// comparison built that way silently matches nothing.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Destructive => "destructive",
            Self::Lifecycle => "lifecycle",
            Self::Task => "task",
            Self::Config => "config",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "destructive" => Self::Destructive,
            "lifecycle" => Self::Lifecycle,
            "task" => Self::Task,
            _ => Self::Config,
        }
    }
}

/// Who caused it.
///
/// The app's own repairs are recorded as well as the user's clicks: somebody
/// looking at a restarted container needs to tell "I did that" from
/// "self-healing did that".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityActor {
    User,
    App,
}

impl ActivityActor {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::App => "app",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "app" {
            Self::App
        } else {
            Self::User
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityOutcome {
    Ok,
    Failed,
    /// Refused before it ran — a guard, or a confirmation declined.
    Denied,
    /// Started, then stopped because the user asked it to.
    ///
    /// Distinct from both neighbours on purpose. `Failed` would blame the
    /// machine for a choice the user made, and `Denied` says it never ran —
    /// but a cancelled image export did run, did take time, and may have left
    /// the work half-done. Background transfers are the first thing here that
    /// can be stopped mid-flight, which is why the vocabulary needed this.
    Cancelled,
}

impl ActivityOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "failed" => Self::Failed,
            "denied" => Self::Denied,
            "cancelled" => Self::Cancelled,
            _ => Self::Ok,
        }
    }
}

/// One thing that happened, as the caller reports it.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub kind: ActivityKind,
    /// `prune`, `remove`, `start`, `pull` — what was done.
    pub verb: String,
    /// `image`, `container`, `volume`, `network`, `instance`, `config`.
    pub target_kind: String,
    /// The stable id, so a row can still be traced after a rename.
    pub target: String,
    /// What a person would call it, which may no longer exist on the machine.
    pub target_name: String,
    pub actor: ActivityActor,
    pub outcome: ActivityOutcome,
    /// A short sentence. Redacted on the way in — never raw argv.
    pub detail: String,
    /// Milliseconds, for `Task` only. `None` everywhere else.
    pub duration_ms: Option<i64>,
}

impl ActivityEntry {
    /// The parts every entry needs, with the rest left at their quiet defaults.
    ///
    /// A builder rather than a struct literal at each call site: thirty
    /// literals is thirty chances to paste the wrong `kind` beside the right
    /// `verb`, and the compiler cannot tell those apart.
    pub fn new(kind: ActivityKind, verb: &str, target_kind: &str, target: &str) -> Self {
        Self {
            kind,
            verb: verb.into(),
            target_kind: target_kind.into(),
            target: target.into(),
            target_name: String::new(),
            actor: ActivityActor::User,
            outcome: ActivityOutcome::Ok,
            detail: String::new(),
            duration_ms: None,
        }
    }

    /// The name a person would recognise, when the caller knows it.
    pub fn named(mut self, name: &str) -> Self {
        self.target_name = name.into();
        self
    }

    /// Mark this as something the app decided to do, not the user.
    pub fn by(mut self, actor: ActivityActor) -> Self {
        self.actor = actor;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// How long it took. Only meaningful for [`ActivityKind::Task`].
    pub fn took(mut self, ms: i64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Read the outcome off the action's own result.
    ///
    /// On failure the error becomes the detail unless the caller already wrote
    /// one, which is what makes "record failures too" the default rather than
    /// something each of thirty call sites has to remember.
    pub fn outcome_of<T, E: std::fmt::Display>(mut self, result: &Result<T, E>) -> Self {
        match result {
            Ok(_) => self.outcome = ActivityOutcome::Ok,
            Err(e) => {
                self.outcome = ActivityOutcome::Failed;
                if self.detail.is_empty() {
                    self.detail = e.to_string();
                }
            }
        }
        self
    }
}

/// What a prune actually did, pulled out of the runtime's own report.
///
/// A row saying "prune ok" with no numbers is useless — the number is the
/// whole reason somebody comes back to look. Docker prints the deleted entries
/// one per line and finishes with a reclaimed-space total.
pub fn prune_summary(stdout: &str) -> String {
    let reclaimed = stdout
        .lines()
        .find(|l| l.starts_with("Total reclaimed space:"))
        .map(|l| l.trim().to_string());
    let deleted = stdout
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty()
                && !l.starts_with("Total reclaimed space:")
                && !l.ends_with(':') // section headers like "Deleted Images:"
        })
        .count();

    match reclaimed {
        Some(total) => format!("{deleted} entries removed. {total}"),
        // Nothing to reclaim is a real answer, and worth recording as one.
        None if deleted == 0 => "nothing to remove".into(),
        None => format!("{deleted} entries removed"),
    }
}

/// A stored row, with what the store added to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRow {
    pub id: i64,
    pub ts: i64,
    pub kind: ActivityKind,
    pub verb: String,
    pub target_kind: String,
    pub target: String,
    pub target_name: String,
    pub actor: ActivityActor,
    pub outcome: ActivityOutcome,
    pub detail: String,
    pub duration_ms: Option<i64>,
}

/// What to narrow a read down to. Every field absent means "everything".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFilter {
    pub kind: Option<String>,
    pub target: Option<String>,
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub limit: Option<i64>,
}

pub fn db_path() -> PathBuf {
    crate::path_util::app_data_dir().join("activity.db")
}

/// The one connection this store writes through.
///
/// Shared rather than reopened per call: recording sits on the path of every
/// user action, and opening a file to write one row would be the slowest part
/// of pressing a button. Nothing here holds the lock
/// for long — every write is a single `INSERT`, and retention is a handful of
/// `DELETE`s every fiftieth one.
static DB: LazyLock<Mutex<Option<Connection>>> = LazyLock::new(|| Mutex::new(None));

fn open_at(path: &std::path::Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("Cannot open activity.db: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| format!("Cannot configure activity.db: {e}"))?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS activity_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            kind TEXT NOT NULL,
            verb TEXT NOT NULL,
            target_kind TEXT NOT NULL DEFAULT '',
            target TEXT NOT NULL DEFAULT '',
            target_name TEXT NOT NULL DEFAULT '',
            actor TEXT NOT NULL,
            outcome TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            duration_ms INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_activity_ts ON activity_log(ts);
        CREATE INDEX IF NOT EXISTS idx_activity_kind_ts ON activity_log(kind, ts);
        CREATE INDEX IF NOT EXISTS idx_activity_target ON activity_log(target, ts);
        ",
    )
    .map_err(|e| format!("Cannot create activity tables: {e}"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Run `f` against the store, opening it on first use.
fn with_db<T>(f: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
    let mut guard = DB.lock().map_err(|_| "activity database is poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(open_at(&db_path())?);
    }
    let conn = guard.as_mut().ok_or_else(|| "activity database is unavailable".to_string())?;
    f(conn)
}

/// Write one action down. Never fails upward — see the module docblock.
pub fn record(entry: ActivityEntry) {
    if let Err(e) = with_db(|conn| insert(conn, &entry, now_ms())) {
        eprintln!("[Activity] could not record {}: {e}", entry.verb);
        return;
    }

    // Opportunistic, and after the write rather than before it: pruning is
    // never the reason an action's record is late.
    if WRITES.fetch_add(1, Ordering::Relaxed).is_multiple_of(RETENTION_EVERY) {
        if let Err(e) = with_db(|conn| run_retention(conn, now_ms())) {
            eprintln!("[Activity] retention failed: {e}");
        }
    }
}

/// The insert itself, separated so tests can drive it against a temp database.
fn insert(conn: &mut Connection, entry: &ActivityEntry, ts: i64) -> Result<(), String> {
    // Redacted here rather than at the call site: thirty callers each
    // remembering to do it is thirty chances to forget, and the one that
    // forgets is the one that logs a password.
    let detail = crate::redact::redact(&entry.detail);
    conn.execute(
        "INSERT INTO activity_log
         (ts, kind, verb, target_kind, target, target_name, actor, outcome, detail, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            ts,
            entry.kind.as_str(),
            entry.verb,
            entry.target_kind,
            entry.target,
            entry.target_name,
            entry.actor.as_str(),
            entry.outcome.as_str(),
            detail,
            entry.duration_ms,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete what this install is no longer keeping.
///
/// Both bounds apply together: whichever is reached first is the one that cuts.
fn run_retention(conn: &mut Connection, now: i64) -> Result<usize, String> {
    let day_ms = 24 * 60 * 60 * 1000i64;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut removed = 0usize;

    removed += tx
        .execute(
            "DELETE FROM activity_log WHERE ts < ?1",
            [now - RETENTION_DAYS * day_ms],
        )
        .map_err(|e| e.to_string())?;
    removed += tx
        .execute(
            "DELETE FROM activity_log WHERE id NOT IN (
                 SELECT id FROM activity_log ORDER BY ts DESC, id DESC LIMIT ?1
             )",
            [MAX_ROWS],
        )
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(removed)
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<ActivityRow> {
    let kind: String = r.get(2)?;
    let actor: String = r.get(7)?;
    let outcome: String = r.get(8)?;
    Ok(ActivityRow {
        id: r.get(0)?,
        ts: r.get(1)?,
        kind: ActivityKind::parse(&kind),
        verb: r.get(3)?,
        target_kind: r.get(4)?,
        target: r.get(5)?,
        target_name: r.get(6)?,
        actor: ActivityActor::parse(&actor),
        outcome: ActivityOutcome::parse(&outcome),
        detail: r.get(9)?,
        duration_ms: r.get(10)?,
    })
}

/// Read the log back, newest first.
pub fn query(filter: &ActivityFilter) -> Result<Vec<ActivityRow>, String> {
    with_db(|conn| select(conn, filter))
}

fn select(conn: &Connection, filter: &ActivityFilter) -> Result<Vec<ActivityRow>, String> {
    let mut sql = String::from(
        "SELECT id, ts, kind, verb, target_kind, target, target_name, actor, outcome, detail, duration_ms
         FROM activity_log WHERE 1 = 1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(kind) = &filter.kind {
        sql.push_str(" AND kind = ?");
        args.push(Box::new(kind.clone()));
    }
    if let Some(target) = &filter.target {
        sql.push_str(" AND target = ?");
        args.push(Box::new(target.clone()));
    }
    if let Some(from) = filter.from_ms {
        sql.push_str(" AND ts >= ?");
        args.push(Box::new(from));
    }
    if let Some(to) = filter.to_ms {
        sql.push_str(" AND ts <= ?");
        args.push(Box::new(to));
    }
    sql.push_str(" ORDER BY ts DESC, id DESC LIMIT ?");
    args.push(Box::new(filter.limit.unwrap_or(200).clamp(1, 1000)));

    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(refs.as_slice(), row_to_entry)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ===== Commands =====

#[tauri::command]
pub async fn activity_query(
    filter: Option<ActivityFilter>,
) -> Result<Vec<ActivityRow>, crate::error::ColimaError> {
    let filter = filter.unwrap_or_default();
    crate::helpers::run_blocking(move || query(&filter))
        .await
        .map_err(crate::error::ColimaError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        create_schema(&conn).expect("schema");
        conn
    }

    fn entry(verb: &str, detail: &str) -> ActivityEntry {
        ActivityEntry {
            kind: ActivityKind::Destructive,
            verb: verb.into(),
            target_kind: "image".into(),
            target: "sha256:abc".into(),
            target_name: "nginx:1.25".into(),
            actor: ActivityActor::User,
            outcome: ActivityOutcome::Ok,
            detail: detail.into(),
            duration_ms: None,
        }
    }

    #[test]
    fn a_row_reads_back_field_for_field() {
        let mut conn = temp_conn();
        let mut e = entry("prune", "removed 4 images, 1.2 GB");
        e.duration_ms = Some(1234);
        e.outcome = ActivityOutcome::Failed;
        e.actor = ActivityActor::App;
        e.kind = ActivityKind::Task;
        insert(&mut conn, &e, 5_000).expect("insert");

        let rows = select(&conn, &ActivityFilter::default()).expect("select");
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.ts, 5_000);
        assert_eq!(r.kind, ActivityKind::Task);
        assert_eq!(r.verb, "prune");
        assert_eq!(r.target_kind, "image");
        assert_eq!(r.target, "sha256:abc");
        assert_eq!(r.target_name, "nginx:1.25");
        assert_eq!(r.actor, ActivityActor::App);
        assert_eq!(r.outcome, ActivityOutcome::Failed);
        assert_eq!(r.detail, "removed 4 images, 1.2 GB");
        assert_eq!(r.duration_ms, Some(1234));
    }

    /// A failure must keep its reason, whichever order the builder is called in.
    ///
    /// The prune call sites compute their detail from the success value, which
    /// is an empty string when the command failed. Written as
    /// `.outcome_of(&r).detail(summary)` that empty string lands *after* the
    /// error message and erases it — the row then says a prune failed without
    /// saying why, which is the one thing a failed row is for.
    #[test]
    fn a_computed_detail_never_erases_the_reason_a_command_failed() {
        let failed: Result<String, String> = Err("docker image prune failed: daemon gone".into());
        // What the call sites do: a summary derived from the success value.
        let summary = failed.as_deref().map(prune_summary).unwrap_or_default();
        assert!(summary.is_empty(), "a failure has no summary to show");

        let entry = ActivityEntry::new(ActivityKind::Destructive, "prune", "image", "")
            .detail(summary)
            .outcome_of(&failed);

        assert_eq!(entry.outcome, ActivityOutcome::Failed);
        assert!(
            entry.detail.contains("daemon gone"),
            "the reason must survive, got: {:?}",
            entry.detail
        );
    }

    /// The success path still shows the numbers rather than nothing.
    #[test]
    fn a_successful_prune_reports_what_it_removed() {
        let ok: Result<String, String> =
            Ok("deleted: sha256:aaa\ndeleted: sha256:bbb\nTotal reclaimed space: 1.2GB".into());
        let summary = ok.as_deref().map(prune_summary).unwrap_or_default();

        let entry = ActivityEntry::new(ActivityKind::Destructive, "prune", "image", "")
            .detail(summary)
            .outcome_of(&ok);

        assert_eq!(entry.outcome, ActivityOutcome::Ok);
        assert!(entry.detail.contains("1.2GB"), "got: {:?}", entry.detail);
    }

    /// A store that cannot be written to must not take the action down with it.
    ///
    /// Two halves, proven differently. That the *caller* is unaffected is
    /// structural: `record` returns `()`, so a failed write has no channel back
    /// to `remove_container` even in principle — no test can strengthen that.
    /// What a test can show is the other half: the failure arrives as an `Err`
    /// to be swallowed rather than as a panic that unwinds through the caller.
    ///
    /// Driven against a connection with no schema, which is what an unwritable
    /// or half-created store looks like from here.
    #[test]
    fn a_store_that_cannot_be_written_to_fails_quietly() {
        let mut conn = Connection::open_in_memory().expect("open");
        // Deliberately no `create_schema`: the table is missing.
        let outcome = insert(&mut conn, &entry("remove", "gone"), 1);
        assert!(
            outcome.is_err(),
            "a missing table must surface as Err, which `record` swallows"
        );

        // And the swallowing itself. The annotation is the assertion: this
        // stops compiling the day `record` starts returning something a caller
        // could be obliged to handle. Calling it also exercises the real path
        // against the real store, so a panic in there fails this test.
        let _: () = record(entry("remove", "gone"));
    }

    #[test]
    fn a_secret_in_the_detail_never_reaches_the_disk() {
        let mut conn = temp_conn();
        insert(&mut conn, &entry("run", "started with PASSWORD=hunter2"), 1).expect("insert");

        let stored: String = conn
            .query_row("SELECT detail FROM activity_log", [], |r| r.get(0))
            .expect("detail");
        assert!(
            !stored.contains("hunter2"),
            "the secret survived redaction: {stored}"
        );
    }

    #[test]
    fn a_failed_action_is_still_readable() {
        let mut conn = temp_conn();
        let mut e = entry("prune", "refused");
        e.outcome = ActivityOutcome::Denied;
        insert(&mut conn, &e, 1).expect("insert");

        let rows = select(&conn, &ActivityFilter::default()).expect("select");
        assert_eq!(rows.len(), 1, "a denied action must not be filtered away");
        assert_eq!(rows[0].outcome, ActivityOutcome::Denied);
    }

    #[test]
    fn keeps_only_the_five_hundred_newest() {
        let mut conn = temp_conn();
        for i in 0..600 {
            insert(&mut conn, &entry("start", "x"), 1_000_000 + i).expect("insert");
        }
        run_retention(&mut conn, 1_000_000 + 600).expect("retention");

        let kept: i64 = conn
            .query_row("SELECT COUNT(*) FROM activity_log", [], |r| r.get(0))
            .expect("count");
        assert_eq!(kept, MAX_ROWS);

        // The newest survive, so the oldest are what went.
        let oldest: i64 = conn
            .query_row("SELECT MIN(ts) FROM activity_log", [], |r| r.get(0))
            .expect("min");
        assert_eq!(oldest, 1_000_000 + 100);
    }

    #[test]
    fn drops_anything_older_than_a_week() {
        let mut conn = temp_conn();
        let day = 24 * 60 * 60 * 1000i64;
        let now = 100 * day;
        insert(&mut conn, &entry("start", "old"), now - 8 * day).expect("insert");
        insert(&mut conn, &entry("start", "new"), now - day).expect("insert");

        run_retention(&mut conn, now).expect("retention");

        let rows = select(&conn, &ActivityFilter::default()).expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].detail, "new");
    }

    #[test]
    fn everything_is_cut_past_the_paid_ceiling() {
        let mut conn = temp_conn();
        let day = 24 * 60 * 60 * 1000i64;
        let now = 500 * day;
        insert(&mut conn, &entry("prune", "ancient"), now - 400 * day).expect("insert");

        run_retention(&mut conn, now).expect("retention");

        assert!(
            select(&conn, &ActivityFilter::default()).expect("select").is_empty(),
            "a row past the year ceiling must go even for a paid install"
        );
    }

    #[test]
    fn a_filter_narrows_by_kind_target_and_time() {
        let mut conn = temp_conn();
        let mut destructive = entry("remove", "gone");
        destructive.target = "sha256:one".into();
        let mut lifecycle = entry("start", "up");
        lifecycle.kind = ActivityKind::Lifecycle;
        lifecycle.target = "sha256:two".into();

        insert(&mut conn, &destructive, 1_000).expect("insert");
        insert(&mut conn, &lifecycle, 2_000).expect("insert");

        let by_kind = select(
            &conn,
            &ActivityFilter { kind: Some("lifecycle".into()), ..Default::default() },
        )
        .expect("by kind");
        assert_eq!(by_kind.len(), 1);
        assert_eq!(by_kind[0].verb, "start");

        let by_target = select(
            &conn,
            &ActivityFilter { target: Some("sha256:one".into()), ..Default::default() },
        )
        .expect("by target");
        assert_eq!(by_target.len(), 1);
        assert_eq!(by_target[0].verb, "remove");

        let by_time = select(
            &conn,
            &ActivityFilter { from_ms: Some(1_500), ..Default::default() },
        )
        .expect("by time");
        assert_eq!(by_time.len(), 1);
        assert_eq!(by_time[0].verb, "start");
    }

    #[test]
    fn the_newest_row_is_read_first() {
        let mut conn = temp_conn();
        insert(&mut conn, &entry("first", "a"), 1_000).expect("insert");
        insert(&mut conn, &entry("second", "b"), 2_000).expect("insert");

        let rows = select(&conn, &ActivityFilter::default()).expect("select");
        assert_eq!(rows[0].verb, "second");
    }

    #[test]
    fn recording_a_thousand_actions_is_not_something_a_user_would_feel() {
        let mut conn = temp_conn();
        let start = std::time::Instant::now();
        for i in 0..1000 {
            insert(&mut conn, &entry("start", "x"), i).expect("insert");
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "1000 records took {elapsed:?}; recording sits on every user action"
        );
    }
}
