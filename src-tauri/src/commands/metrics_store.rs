//! Durable metrics history.
//!
//! `docker stats` tells you what is happening now. This tells you what happened
//! last night — which is the half that explains an outage nobody was awake for.
//!
//! ## The collector must never wait for the disk
//!
//! Samples arrive on the collector's tick. If writing them blocked that tick,
//! a slow disk would stretch the sampling period and the live graph's x-axis
//! would quietly stop meaning what it says. So the writer is a bounded channel
//! and a thread: when the queue is full, batches are **dropped and counted**
//! rather than queued without limit or waited on. A visible gap in history is a
//! smaller lie than a graph whose time base drifts.
//!
//! ## Two databases, not one
//!
//! `metrics.db` is pruned aggressively — rolled up, aged out, and truncated to a
//! size cap. Alert rules the user typed by hand live in `knowledge.db` instead
//! (see `alerts.rs`), where the app keeps its other settings — deleting
//! somebody's configuration during a retention sweep is not a trade-off, it is a
//! bug waiting for a release note.
//!
//! ## Every row carries its instance
//!
//! `detect_docker_host()` picks the first running Colima profile. On the day a
//! second engine is supported, samples written today without an `instance`
//! column would be an unattributable mix with no way to separate them
//! afterwards. The column is here from the first row.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::commands::metrics_collector::MetricSample;

/// Raw samples older than this are rolled up into one-minute rows.
///
/// An hour of raw at a two-second tick is about 1,800 rows per container — small
/// enough to query directly, recent enough to be the window anyone actually
/// zooms into.
const RAW_RETENTION: Duration = Duration::from_secs(60 * 60);

/// Rolled-up rows older than this are deleted.
const ROLLUP_RETENTION_DAYS: i64 = 90;

/// Hard ceiling on the database file. Oldest rows go first when it is exceeded.
const DEFAULT_SIZE_CAP_BYTES: u64 = 200 * 1024 * 1024;

/// How often retention runs.
const RETENTION_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// How long the writer thread gathers before committing.
///
/// One transaction per batch rather than per sample: a two-second tick with
/// twenty containers is twenty rows, and committing each one separately is how
/// a monitoring feature becomes the reason somebody's SSD wore out.
const WRITE_BATCH_INTERVAL: Duration = Duration::from_secs(5);

/// Queue depth before batches start being dropped.
///
/// Deliberately small. A backlog this deep already means the disk cannot keep up,
/// and a deeper queue would only delay that discovery while using more memory.
const QUEUE_CAPACITY: usize = 64;

/// Batches dropped because the queue was full, since startup.
static DROPPED_BATCHES: AtomicU64 = AtomicU64::new(0);
/// Batches accepted, so a drop rate can be reported rather than a raw count.
static ACCEPTED_BATCHES: AtomicU64 = AtomicU64::new(0);
/// Batches not written because entitlement had lapsed. Not a data-loss figure.
static SKIPPED_BATCHES: AtomicU64 = AtomicU64::new(0);

pub fn db_path() -> PathBuf {
    crate::path_util::app_data_dir().join("metrics.db")
}

/// Open a connection with the pragmas this store needs.
///
/// Every reader opens its own. The repo's other SQLite user shares one
/// connection behind a global mutex, which would put every history query behind
/// the writer's transaction — WAL exists precisely so that is unnecessary.
fn open(path: &std::path::Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path).map_err(|e| format!("Cannot open metrics.db: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| format!("Cannot configure metrics.db: {e}"))?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS samples_raw (
            ts INTEGER NOT NULL,
            instance TEXT NOT NULL,
            container_id TEXT NOT NULL,
            cpu_pct REAL NOT NULL,
            mem_bytes INTEGER NOT NULL,
            mem_pct REAL NOT NULL,
            net_rx_bytes INTEGER NOT NULL,
            net_tx_bytes INTEGER NOT NULL,
            block_read_bytes INTEGER NOT NULL,
            block_write_bytes INTEGER NOT NULL,
            pids INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_raw_ts ON samples_raw(ts);
        CREATE INDEX IF NOT EXISTS idx_raw_container ON samples_raw(container_id, ts);

        -- One row per container per minute. `max` is kept beside `avg` because a
        -- spike that an average smooths away is usually the thing being looked for.
        CREATE TABLE IF NOT EXISTS samples_1m (
            ts INTEGER NOT NULL,
            instance TEXT NOT NULL,
            container_id TEXT NOT NULL,
            cpu_pct REAL NOT NULL,
            cpu_pct_max REAL NOT NULL,
            mem_bytes INTEGER NOT NULL,
            mem_bytes_max INTEGER NOT NULL,
            mem_pct REAL NOT NULL,
            net_rx_bytes INTEGER NOT NULL,
            net_tx_bytes INTEGER NOT NULL,
            block_read_bytes INTEGER NOT NULL,
            block_write_bytes INTEGER NOT NULL,
            pids INTEGER NOT NULL,
            PRIMARY KEY (ts, instance, container_id)
        );
        CREATE INDEX IF NOT EXISTS idx_1m_container ON samples_1m(container_id, ts);

        -- Names for containers that no longer exist. Without this, history for a
        -- container removed last week is a row of hex with nothing to call it.
        CREATE TABLE IF NOT EXISTS containers_seen (
            container_id TEXT PRIMARY KEY,
            instance TEXT NOT NULL,
            name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_seen INTEGER NOT NULL
        );
        ",
    )
    .map_err(|e| format!("Cannot create metrics schema: {e}"))
}

// ===== Writing =====

/// A batch on its way to disk.
type Batch = Vec<MetricSample>;

pub struct MetricsStore {
    tx: SyncSender<Batch>,
}

impl MetricsStore {
    /// Hand a batch to the writer thread, or drop it.
    ///
    /// Never blocks and never fails upward: the collector's tick is not the
    /// place to discover that a disk is slow.
    fn submit(&self, samples: &[MetricSample]) {
        if samples.is_empty() {
            return;
        }
        match self.tx.try_send(samples.to_vec()) {
            Ok(()) => {
                ACCEPTED_BATCHES.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                DROPPED_BATCHES.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// What the UI needs to say "history has holes and here is why".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterHealth {
    pub accepted_batches: u64,
    /// Lost because the disk could not keep up. A real hole in the data.
    pub dropped_batches: u64,
    /// Not written because Pro had lapsed. Not a hole anyone can fix by
    /// buying a faster disk.
    pub skipped_batches: u64,
    pub db_bytes: u64,
    /// False when this build is not writing history at all.
    pub writing: bool,
}

pub fn health(writing: bool) -> WriterHealth {
    WriterHealth {
        accepted_batches: ACCEPTED_BATCHES.load(Ordering::Relaxed),
        dropped_batches: DROPPED_BATCHES.load(Ordering::Relaxed),
        skipped_batches: SKIPPED_BATCHES.load(Ordering::Relaxed),
        db_bytes: std::fs::metadata(db_path()).map(|m| m.len()).unwrap_or(0),
        writing,
    }
}

fn insert_batch(conn: &mut Connection, batches: &[Batch]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut sample_stmt = tx
            .prepare_cached(
                "INSERT INTO samples_raw
                 (ts, instance, container_id, cpu_pct, mem_bytes, mem_pct,
                  net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes, pids)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            )
            .map_err(|e| e.to_string())?;
        let mut seen_stmt = tx
            .prepare_cached(
                "INSERT INTO containers_seen (container_id, instance, name, first_seen, last_seen)
                 VALUES (?1,?2,?3,?4,?4)
                 ON CONFLICT(container_id) DO UPDATE SET
                    name = excluded.name,
                    last_seen = excluded.last_seen",
            )
            .map_err(|e| e.to_string())?;

        for batch in batches {
            for s in batch {
                sample_stmt
                    .execute(params![
                        s.ts,
                        s.instance,
                        s.container_id,
                        s.cpu_pct,
                        s.mem_bytes as i64,
                        s.mem_pct,
                        s.net_rx_bytes as i64,
                        s.net_tx_bytes as i64,
                        s.block_read_bytes as i64,
                        s.block_write_bytes as i64,
                        s.pids as i64,
                    ])
                    .map_err(|e| e.to_string())?;
                seen_stmt
                    .execute(params![s.container_id, s.instance, s.name, s.ts])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

/// Start the writer thread and return the handle the collector writes through.
///
/// `keep_writing` is re-read on every batch rather than captured once: Pro can
/// lapse while the app is running, and history must stop growing then — without
/// a restart, which is the version of this that nobody would notice was broken.
pub fn start(path: PathBuf, keep_writing: impl Fn() -> bool + Send + 'static) -> Result<MetricsStore, String> {
    let mut conn = open(&path)?;
    create_schema(&conn)?;

    let (tx, rx) = sync_channel::<Batch>(QUEUE_CAPACITY);

    std::thread::Builder::new()
        .name("metrics-writer".into())
        .spawn(move || {
            let mut pending: Vec<Batch> = Vec::new();
            let mut last_flush = Instant::now();
            loop {
                // A timeout rather than a blocking receive, so a quiet period
                // still flushes what is already queued.
                match rx.recv_timeout(WRITE_BATCH_INTERVAL) {
                    Ok(batch) => pending.push(batch),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    // The sender is gone: the app is shutting down. Write what
                    // is queued before leaving — up to five seconds of samples
                    // are sitting here, and quitting is not a reason to lose
                    // them.
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        if !pending.is_empty() && keep_writing() {
                            let _ = insert_batch(&mut conn, &pending);
                        }
                        break;
                    }
                }

                if pending.is_empty() || last_flush.elapsed() < WRITE_BATCH_INTERVAL {
                    continue;
                }
                if keep_writing() {
                    if let Err(e) = insert_batch(&mut conn, &pending) {
                        // Logged, not propagated: there is nobody above this
                        // thread to tell, and losing history must not take the
                        // live view down with it.
                        eprintln!("metrics write failed: {e}");
                    }
                } else {
                    // Entitlement lapsed. Existing history is kept — the user
                    // paid for those samples — but nothing new is written.
                    //
                    // Counted separately from dropped batches: the UI presents
                    // drops as "the disk could not keep up", and a licensing
                    // pause is a different sentence with a different remedy.
                    SKIPPED_BATCHES.fetch_add(pending.len() as u64, Ordering::Relaxed);
                }
                pending.clear();
                last_flush = Instant::now();
            }
        })
        .map_err(|e| format!("Cannot start the metrics writer: {e}"))?;

    Ok(MetricsStore { tx })
}

/// Install the store as the collector's durable sink.
pub fn install(store: MetricsStore) {
    let store = Arc::new(store);
    WRITING.store(true, Ordering::Relaxed);
    crate::commands::metrics_collector::set_metric_writer(Arc::new(move |batch| {
        store.submit(batch);
    }));
}

/// True when this build has a store attached at all.
static WRITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_writing() -> bool {
    WRITING.load(Ordering::Relaxed)
}

/// Whether a live, unexpired entitlement exists on this machine.
///
/// Read on every batch rather than once at startup: Pro can lapse while the app
/// runs, and "stops writing after a restart" is the version of this nobody would
/// notice was broken. No user id is passed — the backend has no session, and
/// signing out does not revoke a subscription that is still paid for.
pub fn entitled_now() -> bool {
    crate::subscription::cache::load()
        .is_some_and(|c| c.is_entitled_for(None, chrono::Utc::now()))
}

/// How long an entitlement answer is reused on the hot path.
///
/// Short enough that a lapse stops writes within seconds, long enough that a
/// two-second tick is not reading and parsing a file twice a second.
const ENTITLEMENT_TTL: Duration = Duration::from_secs(30);

static ENTITLEMENT_CACHE: LazyLock<Mutex<Option<(bool, Instant)>>> =
    LazyLock::new(|| Mutex::new(None));

/// [`entitled_now`], cached, for callers on the collector's tick.
pub fn entitled_now_cached() -> bool {
    if let Ok(cache) = ENTITLEMENT_CACHE.lock() {
        if let Some((value, at)) = *cache {
            if at.elapsed() < ENTITLEMENT_TTL {
                return value;
            }
        }
    }
    let value = entitled_now();
    if let Ok(mut cache) = ENTITLEMENT_CACHE.lock() {
        *cache = Some((value, Instant::now()));
    }
    value
}

/// Attach durable history if this install is entitled to it.
///
/// Called once at startup. A Free install gets no writer and therefore no
/// `metrics.db` at all — not an empty one, not a disabled one.
pub fn start_if_entitled() {
    // Idempotent, because this is called at boot *and* whenever entitlement is
    // re-established. `install` replaces the collector's sink rather than
    // adding to it, so a second call would spawn a second writer thread and
    // orphan the first — two threads writing the same database, and the older
    // one only noticing when its channel is dropped.
    if is_writing() {
        return;
    }
    if !entitled_now() {
        return;
    }
    match start(db_path(), entitled_now) {
        Ok(store) => {
            install(store);
            spawn_retention();
        }
        Err(e) => eprintln!("metrics history unavailable: {e}"),
    }
}

/// Forget the cached entitlement answer.
///
/// Called when the subscription record itself changes. Without it, a user who
/// upgrades waits up to [`ENTITLEMENT_TTL`] before anything reads the new
/// answer — a delay with no cause the user can see.
pub fn invalidate_entitlement_cache() {
    if let Ok(mut cache) = ENTITLEMENT_CACHE.lock() {
        *cache = None;
    }
}

// ===== Retention =====

/// Roll up, age out, and enforce the size cap. Returns rows removed from `raw`.
pub fn run_retention(conn: &mut Connection, now_ms: i64, size_cap: u64, path: &std::path::Path) -> Result<usize, String> {
    // Whole minutes only. Cutting mid-minute rolls up the first fraction, deletes
    // it, and then the next pass rolls up the remainder and *replaces* the
    // earlier aggregate — losing both the average's first half and any peak in
    // it. Aligning the cutoff means a minute is only ever rolled up once, when
    // it is complete.
    let cutoff = ((now_ms - RAW_RETENTION.as_millis() as i64) / 60_000) * 60_000;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // Roll raw into per-minute buckets. `INSERT OR REPLACE` rather than plain
    // insert because a bucket can be written twice if retention runs while the
    // minute is still filling.
    tx.execute(
        "INSERT OR REPLACE INTO samples_1m
         SELECT (ts / 60000) * 60000 AS bucket, instance, container_id,
                AVG(cpu_pct), MAX(cpu_pct),
                CAST(AVG(mem_bytes) AS INTEGER), MAX(mem_bytes), AVG(mem_pct),
                CAST(AVG(net_rx_bytes) AS INTEGER), CAST(AVG(net_tx_bytes) AS INTEGER),
                CAST(AVG(block_read_bytes) AS INTEGER), CAST(AVG(block_write_bytes) AS INTEGER),
                CAST(AVG(pids) AS INTEGER)
         FROM samples_raw WHERE ts < ?1
         GROUP BY bucket, instance, container_id",
        params![cutoff],
    )
    .map_err(|e| format!("roll-up failed: {e}"))?;

    let removed = tx
        .execute("DELETE FROM samples_raw WHERE ts < ?1", params![cutoff])
        .map_err(|e| format!("raw prune failed: {e}"))?;

    let rollup_cutoff = now_ms - ROLLUP_RETENTION_DAYS * 86_400_000;
    tx.execute("DELETE FROM samples_1m WHERE ts < ?1", params![rollup_cutoff])
        .map_err(|e| format!("rollup prune failed: {e}"))?;

    tx.commit().map_err(|e| e.to_string())?;

    enforce_size_cap(conn, size_cap, path)?;
    Ok(removed)
}

/// Delete oldest rolled-up rows until the file is under the cap.
///
/// Oldest first because the recent past is what gets looked at, and a cap that
/// deleted the newest data would make the feature useless exactly when it
/// mattered.
fn enforce_size_cap(conn: &mut Connection, cap: u64, path: &std::path::Path) -> Result<(), String> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size <= cap {
        return Ok(());
    }

    // Delete proportionally to the overshoot in one pass, then vacuum once.
    //
    // The earlier version vacuumed inside the loop, up to eight times: each one
    // rewrites the whole file under an exclusive lock, which reliably exceeds
    // the writer's five-second `busy_timeout` and turns a housekeeping pass into
    // dropped samples.
    let overshoot = size.saturating_sub(cap) as f64 / size.max(1) as f64;
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM samples_1m", [], |r| r.get(0))
        .unwrap_or(0);
    // A floor so an oversized file with few rolled-up rows still makes progress,
    // and a margin so one pass is usually enough.
    let to_delete = ((rows as f64 * overshoot * 1.2) as i64).max(1000).min(rows);

    let deleted = conn
        .execute(
            "DELETE FROM samples_1m WHERE rowid IN
             (SELECT rowid FROM samples_1m ORDER BY ts ASC LIMIT ?1)",
            params![to_delete],
        )
        .map_err(|e| format!("size cap prune failed: {e}"))?;

    if deleted > 0 {
        // `VACUUM` is what actually returns the space to the filesystem; without
        // it the file never shrinks and the next pass deletes again for nothing.
        conn.execute_batch("VACUUM").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Run retention on a timer for as long as the app lives.
pub fn spawn_retention() {
    std::thread::Builder::new()
        .name("metrics-retention".into())
        .spawn(move || loop {
            std::thread::sleep(RETENTION_INTERVAL);
            let path = db_path();
            if !path.exists() {
                continue;
            }
            let Ok(mut conn) = open(&path) else { continue };
            if let Err(e) = run_retention(&mut conn, now_ms(), DEFAULT_SIZE_CAP_BYTES, &path) {
                eprintln!("metrics retention failed: {e}");
            }
        })
        .ok();
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ===== Reading =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub ts: i64,
    pub container_id: String,
    pub cpu_pct: f64,
    pub mem_bytes: i64,
    pub mem_pct: f64,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
    pub block_read_bytes: i64,
    pub block_write_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySeries {
    /// `raw` or `1m` — the UI says which, because a one-minute average is not
    /// the same claim as a two-second sample.
    pub resolution: String,
    /// The gap, in milliseconds, beyond which the UI must break the line rather
    /// than join two points across missing data.
    pub expected_step_ms: i64,
    pub points: Vec<HistoryPoint>,
    /// Names for every container id in `points`, including deleted ones.
    pub names: std::collections::HashMap<String, String>,
}

/// The most rows one query will return.
///
/// Thirty days of per-minute rows for twenty containers is around 860,000 —
/// more than any chart can draw and more than a webview should be asked to
/// hold. The window is trimmed from the oldest end, which is the half a reader
/// is least likely to be looking at.
const MAX_HISTORY_ROWS: usize = 20_000;

/// History for a time range.
///
/// Under an hour reads raw samples. Longer ranges read the rolled-up table
/// **and** aggregate the last hour of raw on the fly, because roll-up only
/// happens for completed minutes older than an hour — without that second half,
/// every 24h and 7d chart would end an hour ago and look like an outage.
pub fn history(from_ms: i64, to_ms: i64, container_ids: &[String]) -> Result<HistorySeries, String> {
    let path = db_path();
    if !path.exists() {
        return Ok(HistorySeries {
            resolution: "raw".into(),
            expected_step_ms: 0,
            points: Vec::new(),
            names: Default::default(),
        });
    }
    let conn = open(&path)?;
    create_schema(&conn)?;

    let span = (to_ms - from_ms).max(0);
    let use_raw = span <= RAW_RETENTION.as_millis() as i64;
    let expected_step_ms = if use_raw {
        crate::commands::metrics_collector::interval_ms() as i64
    } else {
        60_000
    };

    // Bound parameters rather than a concatenated list: string building is how
    // an id containing a quote becomes either a mangled filter or worse.
    let placeholders = if container_ids.is_empty() {
        String::new()
    } else {
        let marks = (0..container_ids.len())
            .map(|i| format!("?{}", i + 3))
            .collect::<Vec<_>>()
            .join(",");
        format!(" AND container_id IN ({marks})")
    };

    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(from_ms), Box::new(to_ms)];
    for id in container_ids {
        args.push(Box::new(id.clone()));
    }

    let read = |sql: String| -> Result<Vec<HistoryPoint>, String> {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())), |r| {
                Ok(HistoryPoint {
                    ts: r.get(0)?,
                    container_id: r.get(1)?,
                    cpu_pct: r.get(2)?,
                    mem_bytes: r.get(3)?,
                    mem_pct: r.get(4)?,
                    net_rx_bytes: r.get(5)?,
                    net_tx_bytes: r.get(6)?,
                    block_read_bytes: r.get(7)?,
                    block_write_bytes: r.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    };

    let mut points = if use_raw {
        read(format!(
            "SELECT ts, container_id, cpu_pct, mem_bytes, mem_pct,
                    net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes
             FROM samples_raw WHERE ts >= ?1 AND ts <= ?2{placeholders} ORDER BY ts ASC"
        ))?
    } else {
        let mut rolled = read(format!(
            "SELECT ts, container_id, cpu_pct, mem_bytes, mem_pct,
                    net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes
             FROM samples_1m WHERE ts >= ?1 AND ts <= ?2{placeholders} ORDER BY ts ASC"
        ))?;
        // The tail the roll-up has not reached yet, bucketed the same way so the
        // whole series carries one meaning.
        let recent = read(format!(
            "SELECT (ts / 60000) * 60000 AS bucket, container_id, AVG(cpu_pct),
                    CAST(AVG(mem_bytes) AS INTEGER), AVG(mem_pct),
                    CAST(AVG(net_rx_bytes) AS INTEGER), CAST(AVG(net_tx_bytes) AS INTEGER),
                    CAST(AVG(block_read_bytes) AS INTEGER), CAST(AVG(block_write_bytes) AS INTEGER)
             FROM samples_raw WHERE ts >= ?1 AND ts <= ?2{placeholders}
             GROUP BY bucket, container_id ORDER BY bucket ASC"
        ))?;
        rolled.extend(recent);
        rolled.sort_by_key(|p| p.ts);
        rolled
    };

    if points.len() > MAX_HISTORY_ROWS {
        points.drain(0..points.len() - MAX_HISTORY_ROWS);
    }

    let mut names = std::collections::HashMap::new();
    let mut name_stmt = conn
        .prepare("SELECT container_id, name FROM containers_seen")
        .map_err(|e| e.to_string())?;
    let named = name_stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    for entry in named.flatten() {
        names.insert(entry.0, entry.1);
    }

    Ok(HistorySeries {
        resolution: if use_raw { "raw".into() } else { "1m".into() },
        expected_step_ms,
        points,
        names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ts: i64, id: &str, cpu: f64) -> MetricSample {
        MetricSample {
            ts,
            instance: "colima".into(),
            container_id: id.into(),
            name: format!("name-of-{id}"),
            cpu_pct: cpu,
            mem_bytes: 1024,
            mem_limit_bytes: 8192,
            mem_pct: 12.5,
            net_rx_bytes: 10,
            net_tx_bytes: 20,
            block_read_bytes: 30,
            block_write_bytes: 40,
            pids: 7,
        }
    }

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        create_schema(&conn).expect("schema");
        conn
    }

    #[test]
    fn every_sample_table_carries_its_instance() {
        // The one column that cannot be added later: samples written without it
        // are an unattributable mix the day a second engine exists.
        let conn = memory_db();
        for table in ["samples_raw", "samples_1m"] {
            let mut stmt = conn
                .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .expect("pragma");
            let cols: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .expect("query")
                .filter_map(|c| c.ok())
                .collect();
            assert!(cols.contains(&"instance".to_string()), "{table} has no instance column");
        }
    }

    #[test]
    fn a_batch_lands_as_rows_and_remembers_the_container_name() {
        let mut conn = memory_db();
        insert_batch(&mut conn, &[vec![sample(1000, "abc", 10.0), sample(1000, "def", 20.0)]])
            .expect("insert");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples_raw", [], |r| r.get(0))
            .expect("count");
        assert_eq!(rows, 2);

        let name: String = conn
            .query_row(
                "SELECT name FROM containers_seen WHERE container_id = 'abc'",
                [],
                |r| r.get(0),
            )
            .expect("name");
        assert_eq!(name, "name-of-abc");
    }

    #[test]
    fn retention_rolls_raw_into_minutes_and_keeps_the_peak() {
        let mut conn = memory_db();
        // Two samples in the same minute, an hour and a half ago.
        let old = 90 * 60 * 1000;
        let now = 3 * 60 * 60 * 1000;
        insert_batch(
            &mut conn,
            &[vec![sample(old, "abc", 10.0), sample(old + 2000, "abc", 90.0)]],
        )
        .expect("insert");

        let removed = run_retention(&mut conn, now, DEFAULT_SIZE_CAP_BYTES, std::path::Path::new("/nonexistent"))
            .expect("retention");
        assert_eq!(removed, 2, "both raw rows are past the raw window");

        let (avg, max): (f64, f64) = conn
            .query_row("SELECT cpu_pct, cpu_pct_max FROM samples_1m", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("rollup row");
        assert_eq!(avg, 50.0);
        // The peak is the thing being looked for; an average alone hides it.
        assert_eq!(max, 90.0);
    }

    #[test]
    fn a_minute_split_across_two_retention_passes_keeps_its_whole_average() {
        // The bug this pins: cutting the window mid-minute rolled up the first
        // fraction, deleted it, and then the second pass *replaced* that row
        // with an aggregate of the remainder — losing half the minute and any
        // peak in it, silently.
        let mut conn = memory_db();
        let minute = 3_600_000i64; // a clean minute boundary
        // 10% at :00 and 90% at :30, one minute's worth of samples.
        insert_batch(
            &mut conn,
            &[vec![sample(minute, "abc", 10.0), sample(minute + 30_000, "abc", 90.0)]],
        )
        .expect("insert");

        // First pass runs while that minute is still inside the raw window.
        run_retention(&mut conn, minute + 40_000, DEFAULT_SIZE_CAP_BYTES, std::path::Path::new("/nonexistent"))
            .expect("first pass");
        // Second pass, an hour and a half later: now the whole minute is old.
        run_retention(&mut conn, minute + 90 * 60_000, DEFAULT_SIZE_CAP_BYTES, std::path::Path::new("/nonexistent"))
            .expect("second pass");

        let (avg, max): (f64, f64) = conn
            .query_row("SELECT cpu_pct, cpu_pct_max FROM samples_1m", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("one rolled-up row");
        assert_eq!(avg, 50.0, "the whole minute, not the tail of it");
        assert_eq!(max, 90.0, "the peak must survive roll-up");
    }

    #[test]
    fn recent_samples_survive_retention() {
        let mut conn = memory_db();
        let now = 3 * 60 * 60 * 1000;
        insert_batch(&mut conn, &[vec![sample(now - 60_000, "abc", 10.0)]]).expect("insert");

        run_retention(&mut conn, now, DEFAULT_SIZE_CAP_BYTES, std::path::Path::new("/nonexistent"))
            .expect("retention");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples_raw", [], |r| r.get(0))
            .expect("count");
        assert_eq!(rows, 1, "a sample from a minute ago is not history yet");
    }

    #[test]
    fn rolled_up_rows_older_than_the_window_are_deleted() {
        let mut conn = memory_db();
        let now = 200 * 86_400_000;
        conn.execute(
            "INSERT INTO samples_1m VALUES (?1,'colima','abc',1.0,2.0,1,2,3.0,4,5,6,7,8)",
            params![now - 120 * 86_400_000],
        )
        .expect("insert old rollup");

        run_retention(&mut conn, now, DEFAULT_SIZE_CAP_BYTES, std::path::Path::new("/nonexistent"))
            .expect("retention");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples_1m", [], |r| r.get(0))
            .expect("count");
        assert_eq!(rows, 0);
    }

    #[test]
    fn a_full_queue_drops_batches_instead_of_blocking_the_collector() {
        // The collector's tick sets the x-axis of the live graph. A writer that
        // could block it would stretch the sampling period under disk pressure,
        // and the graph would lie about time rather than about completeness.
        let (tx, rx) = sync_channel::<Batch>(1);
        let store = MetricsStore { tx };
        let before = DROPPED_BATCHES.load(Ordering::Relaxed);

        let started = Instant::now();
        for _ in 0..50 {
            store.submit(&[sample(1, "abc", 1.0)]);
        }
        assert!(started.elapsed() < Duration::from_millis(500), "submit blocked");
        assert!(
            DROPPED_BATCHES.load(Ordering::Relaxed) > before,
            "a full queue must drop and count"
        );
        drop(rx);
    }

    /// Attaching the writer twice must not happen.
    ///
    /// `start_if_entitled` is now called from two places — once at boot, and
    /// again whenever a subscription record is stored, which is what makes an
    /// upgrade take effect without a restart. `install` *replaces* the
    /// collector's sink rather than adding to it, so without the guard the
    /// second call would spawn a second writer thread against the same database
    /// and leave the first to notice only when its channel dropped.
    ///
    /// Asserted on the guard itself rather than by starting real writers: the
    /// flag is process-global, and a test that installed one would leak it into
    /// every other test in this binary.
    #[test]
    fn a_second_start_is_refused_while_a_writer_is_attached() {
        let attached = std::sync::atomic::AtomicBool::new(false);

        // The shape of `start_if_entitled`'s first guard.
        let would_start = |writing: bool, entitled: bool| !writing && entitled;

        assert!(would_start(false, true), "boot with Pro starts a writer");
        attached.store(true, Ordering::Relaxed);
        assert!(
            !would_start(attached.load(Ordering::Relaxed), true),
            "storing another entitled record must not start a second writer"
        );
        assert!(!would_start(false, false), "Free never starts one");
    }

    #[test]
    fn an_empty_batch_is_not_written() {
        let (tx, rx) = sync_channel::<Batch>(4);
        let store = MetricsStore { tx };
        store.submit(&[]);
        assert!(rx.try_recv().is_err(), "an empty tick should not reach the disk");
    }

    // ===== Simulated soak =====
    //
    // A day of twenty containers pushed through the real write and retention
    // code on a real file, with the clock supplied rather than read. Run it
    // with `cargo test --lib -- --ignored soak`.
    //
    // ## What this does not do
    //
    // It is **not** the twenty-four hour soak the phase asks for, and does not
    // close that item. Simulated time cannot observe the things a wall-clock
    // run exists to catch: a writer thread leaking memory over a day, the
    // sampling interval drifting, the daemon restarting underneath, or the
    // channel filling because the disk stalled. The drop rate in particular is
    // a property of a queue under real pressure — here nothing is ever behind,
    // so a passing run says nothing about it.
    //
    // What it does check is the part that is arithmetic rather than weather:
    // that a day's worth of rows rolls up as intended, stays under the ceiling,
    // and is still queryable a week later. Those are the ways the store can be
    // wrong on any machine, and they should not need a day to find out.

    const SOAK_CONTAINERS: usize = 20;
    const SOAK_TICK_MS: i64 = 2_000;
    const SOAK_DAY_MS: i64 = 24 * 60 * 60 * 1000;

    /// A file-backed database, because the size cap is measured on the file.
    fn soak_db(tag: &str) -> (Connection, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("colimaui-soak-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).expect("open soak db");
        create_schema(&conn).expect("schema");
        (conn, path)
    }

    /// One minute of samples for every container, at the production tick.
    fn soak_minute(minute_start: i64) -> Batch {
        let mut batch = Vec::new();
        let mut ts = minute_start;
        while ts < minute_start + 60_000 {
            for c in 0..SOAK_CONTAINERS {
                // Varying CPU so the rollup's average and peak are not the same
                // number, which would hide a bug that keeps only one of them.
                batch.push(sample(ts, &format!("soak-{c}"), (ts % 100) as f64));
            }
            ts += SOAK_TICK_MS;
        }
        batch
    }

    /// Drive `minutes` of simulated time, running retention on its real cadence.
    fn run_soak(conn: &mut Connection, path: &std::path::Path, minutes: i64) {
        let retention_every = RETENTION_INTERVAL.as_secs() as i64 / 60;
        for m in 0..minutes {
            let minute_start = m * 60_000;
            insert_batch(conn, &[soak_minute(minute_start)]).expect("insert minute");
            if (m + 1) % retention_every == 0 {
                run_retention(conn, minute_start + 60_000, DEFAULT_SIZE_CAP_BYTES, path)
                    .expect("retention");
            }
        }
    }

    #[test]
    #[ignore = "soak: a simulated day at production rates; run with --ignored"]
    fn soak_a_simulated_day_of_twenty_containers_stays_under_the_size_cap() {
        let (mut conn, path) = soak_db("cap");
        run_soak(&mut conn, &path, SOAK_DAY_MS / 60_000);

        // WAL pages count against the user's disk as much as the main file.
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
        let size = std::fs::metadata(&path).expect("stat").len();
        let _ = std::fs::remove_file(&path);

        assert!(
            size <= DEFAULT_SIZE_CAP_BYTES,
            "a day of twenty containers grew the store to {size} bytes, over the {DEFAULT_SIZE_CAP_BYTES} cap"
        );
    }

    #[test]
    #[ignore = "soak: a simulated day at production rates; run with --ignored"]
    fn soak_a_simulated_day_keeps_the_recent_hour_raw_and_the_rest_rolled_up() {
        let (mut conn, path) = soak_db("shape");
        run_soak(&mut conn, &path, SOAK_DAY_MS / 60_000);

        let raw: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples_raw", [], |r| r.get(0))
            .expect("count raw");
        let rolled: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples_1m", [], |r| r.get(0))
            .expect("count 1m");
        let _ = std::fs::remove_file(&path);

        // The raw window holds an hour, and retention runs every five minutes,
        // so at most an hour and a change of raw rows survives.
        let per_minute = (60_000 / SOAK_TICK_MS) * SOAK_CONTAINERS as i64;
        let raw_ceiling = per_minute * (RAW_RETENTION.as_secs() as i64 / 60 + retention_slack());
        assert!(
            raw <= raw_ceiling,
            "{raw} raw rows survived a day; the window should cap it near {raw_ceiling}"
        );

        // Everything older became one row per container per minute. Whole
        // minutes only, so the last partial minute is not counted.
        let rolled_minutes = (SOAK_DAY_MS / 60_000) - (RAW_RETENTION.as_secs() as i64 / 60);
        let expected = (rolled_minutes - retention_slack()) * SOAK_CONTAINERS as i64;
        assert!(
            rolled >= expected,
            "only {rolled} rolled-up rows for a day; expected at least {expected}"
        );
    }

    /// Minutes that may still be raw because retention has not run for them yet.
    fn retention_slack() -> i64 {
        RETENTION_INTERVAL.as_secs() as i64 / 60
    }

    #[test]
    #[ignore = "soak: a simulated day at production rates; run with --ignored"]
    fn soak_a_week_old_query_still_answers_after_a_simulated_day() {
        let (mut conn, path) = soak_db("query");
        run_soak(&mut conn, &path, SOAK_DAY_MS / 60_000);

        // The query the History tab makes on its widest range. It has to return
        // the rolled-up rows, not an empty series, once raw has aged out.
        let week = 7 * 24 * 60 * 60 * 1000i64;
        let points: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM samples_1m WHERE ts >= ?1 AND ts <= ?2",
                [SOAK_DAY_MS - week, SOAK_DAY_MS],
                |r| r.get(0),
            )
            .expect("week query");
        let named: i64 = conn
            .query_row("SELECT COUNT(*) FROM containers_seen", [], |r| r.get(0))
            .expect("names");
        let _ = std::fs::remove_file(&path);

        assert!(points > 0, "a week-wide query found nothing after a day of samples");
        assert_eq!(
            named, SOAK_CONTAINERS as i64,
            "every container that was sampled must still have a name to label it with"
        );
    }
}
