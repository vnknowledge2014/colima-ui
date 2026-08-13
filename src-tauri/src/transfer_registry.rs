//! What transfers exist, independently of the window that started them.
//!
//! `streaming_cmd::RUNNING` already tracks in-flight *processes*, but it is built
//! for killing them: entries appear only once the child is spawned, disappear the
//! moment it is reaped, and carry nothing a user could read. That is the wrong
//! shape for answering "what is going on" after a reload.
//!
//! Three concrete failures this exists to fix:
//!
//!  - **A job could finish before anyone was listening.** `spawn_job` starts the
//!    command before returning the job id, so a spawn failure emitted
//!    `transfer.failed` microseconds later — for an id the caller did not have yet.
//!    Registration happens before the spawn now, and terminal states are kept for
//!    [`RETENTION`] so a client that missed the event can still read the outcome.
//!
//!  - **A reload lost the job.** Nothing could enumerate transfers, so a webview
//!    that reloaded left a `docker save` writing to disk with no way to see or
//!    cancel it. [`list`] is that enumeration.
//!
//!  - **`overwrite: false` was decided too early.** The destination is checked when
//!    a job starts and written when it ends; two jobs aimed at one path both passed
//!    that check. This is the only place that knows about the other job, so the
//!    conflict is refused here.
//!
//! Deliberately **not** persisted: `kill_all_streams` runs on `ExitRequested`, so no
//! transfer outlives the process and a file claiming otherwise would only ever be
//! wrong.

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long a finished transfer stays readable.
///
/// Long enough for a webview to reload and reconcile, short enough that the list
/// stays about *now*. After this a job is simply absent, which callers read as
/// "finished and already reported".
const RETENTION: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferStatus {
    /// Registered, but the child process has not been spawned yet.
    Starting,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl TransferStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }
}

/// What a client can learn about one transfer.
///
/// `target_label` is a name — an image reference, a container path — never a host
/// path. Host paths are what the user chose and are of no use to a viewer who
/// already knows where they saved it; keeping them out means this can be logged and
/// sent over SSE without a redaction pass.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub job_id: String,
    /// `save` | `load` | `cp-in` | `cp-out`, matching the job id prefix.
    pub kind: String,
    pub status: TransferStatus,
    pub bytes: u64,
    pub total_estimate: Option<u64>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub target_label: String,
    /// Redacted failure text, when the job failed.
    pub error: Option<String>,
}

struct Entry {
    snapshot: TransferSnapshot,
    /// Where this job will write, for the conflict check. Never serialised.
    destination: Option<PathBuf>,
    /// Set by [`request_cancel`] before the child exists, so a cancel that arrives
    /// during startup is not silently dropped.
    cancel_requested: bool,
}

static REGISTRY: LazyLock<Mutex<HashMap<String, Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<String, Entry>> {
    // A panic while holding this lock cannot leave the map inconsistent — every
    // mutation is a single insert or field assignment — so poisoning carries no
    // information worth acting on. Discarding the map instead would be actively
    // harmful: `list` would report no transfers and a reconciling client would
    // conclude everything had finished.
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Drop terminal entries older than [`RETENTION`].
///
/// Called from the mutating paths rather than on a timer: the map only grows when
/// something touches it, so that is exactly when it needs trimming.
fn prune(map: &mut HashMap<String, Entry>) {
    let cutoff = now_ms().saturating_sub(RETENTION.as_millis() as u64);
    map.retain(|_, e| match e.snapshot.finished_at {
        Some(at) => at > cutoff,
        None => true,
    });
}

/// Record a transfer that is about to start.
///
/// Fails when another *unfinished* job already writes to `destination`. That check
/// belongs here and nowhere else: `resolve_destination` sees only the filesystem,
/// where the first job's output does not exist yet.
pub fn register(
    job_id: &str,
    kind: &str,
    target_label: &str,
    total_estimate: Option<u64>,
    destination: Option<PathBuf>,
) -> Result<(), String> {
    let mut map = lock_registry();
    prune(&mut map);

    if let Some(dest) = &destination {
        if let Some(other) = map.values().find(|e| {
            !e.snapshot.status.is_terminal() && e.destination.as_deref() == Some(dest.as_path())
        }) {
            return Err(format!(
                "Another transfer ({}) is already writing to {}. Wait for it or cancel it first.",
                other.snapshot.job_id,
                dest.display()
            ));
        }
    }

    map.insert(
        job_id.to_string(),
        Entry {
            snapshot: TransferSnapshot {
                job_id: job_id.to_string(),
                kind: kind.to_string(),
                status: TransferStatus::Starting,
                bytes: 0,
                total_estimate,
                started_at: now_ms(),
                finished_at: None,
                target_label: target_label.to_string(),
                error: None,
            },
            destination,
            cancel_requested: false,
        },
    );
    Ok(())
}

/// Mark the child as spawned. Returns false when the job was cancelled first.
pub fn set_running(job_id: &str) -> bool {
    let mut map = lock_registry();
    match map.get_mut(job_id) {
        Some(entry) if entry.cancel_requested => false,
        Some(entry) => {
            entry.snapshot.status = TransferStatus::Running;
            true
        }
        None => true,
    }
}

pub fn update_bytes(job_id: &str, bytes: u64) {
    if let Some(entry) = lock_registry().get_mut(job_id) {
        entry.snapshot.bytes = bytes;
    }
}

/// Record the outcome.
///
/// Called after the destination has been published, not when the child is reaped —
/// a client that sees `Success` here can rely on the file being there.
pub fn settle(job_id: &str, status: TransferStatus, bytes: u64, error: Option<String>) {
    let mut logged = None;
    {
        let mut map = lock_registry();
        if let Some(entry) = map.get_mut(job_id) {
            entry.snapshot.status = status;
            entry.snapshot.bytes = bytes;
            entry.snapshot.error = error.clone();
            entry.snapshot.finished_at = Some(now_ms());
            // Nothing is being written any more, so the path is free for the next
            // job even while this entry is still readable.
            entry.destination = None;

            let elapsed = now_ms().saturating_sub(entry.snapshot.started_at);
            logged = Some((entry.snapshot.kind.clone(), entry.snapshot.target_label.clone(), elapsed));
        }
        prune(&mut map);
    }

    // Written here, not where the job was started: a record saying "began
    // exporting" is rubbish once the export fails, and this is the one place
    // every transfer passes through exactly once on its way to a real outcome.
    // The registry lock is released first — recording opens a database, and
    // holding this lock across that would put every other job behind it.
    if let Some((kind, label, elapsed)) = logged {
        let outcome = match status {
            TransferStatus::Success => crate::commands::activity::ActivityOutcome::Ok,
            // A cancel is a decision, not a fault: the user asked for it, and a
            // log that reads it as a failure teaches people to ignore failures.
            // Nor is it `Denied`, which says nothing ever ran — a cancelled
            // export did run, took time, and may have left half a file behind.
            TransferStatus::Cancelled => crate::commands::activity::ActivityOutcome::Cancelled,
            _ => crate::commands::activity::ActivityOutcome::Failed,
        };
        let mut entry = crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Task,
            &kind,
            "transfer",
            job_id,
        )
        .named(&label)
        .took(elapsed as i64);
        entry.outcome = outcome;
        if let Some(reason) = error {
            entry.detail = reason;
        } else {
            // Bytes rather than a formatted size: this store is read by a UI that
            // formats for its own locale, and a number never needs reparsing.
            entry.detail = format!("{bytes} bytes transferred");
        }
        crate::commands::activity::record(entry);
    }
}

/// Everything worth showing: unfinished jobs, plus recent outcomes.
pub fn list() -> Vec<TransferSnapshot> {
    let mut map = lock_registry();
    prune(&mut map);
    let mut out: Vec<TransferSnapshot> = map.values().map(|e| e.snapshot.clone()).collect();
    // Newest first, and never dependent on HashMap iteration order.
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    out
}

/// Settles a job if nothing else did, when it goes out of scope.
///
/// The job runner can stop without reaching a terminal branch — a panic inside the
/// blocking task, or the runtime dropping it during shutdown. The entry would then
/// sit at `Running` forever, and because [`prune`] only drops *finished* entries it
/// would hold its destination claim for the life of the process: that path could
/// never be exported to again. Call [`JobGuard::done`] on the normal paths so this
/// only fires when something went wrong.
pub struct JobGuard {
    job_id: String,
    settled: bool,
}

impl JobGuard {
    pub fn new(job_id: &str) -> Self {
        Self {
            job_id: job_id.to_string(),
            settled: false,
        }
    }

    /// The job reached a terminal state on its own; stand down.
    pub fn done(mut self) {
        self.settled = true;
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        settle(
            &self.job_id,
            TransferStatus::Failed,
            0,
            Some("The transfer stopped unexpectedly.".to_string()),
        );
        crate::streaming_cmd::disarm_cancel(&self.job_id);
    }
}

/// What a cancel request actually did.
///
/// A bare `bool` could not tell "there is no such job" from "it had already
/// finished", so callers showed a spinner that never resolved for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CancelOutcome {
    /// The job was running and has been asked to stop. The terminal state still
    /// arrives as an event.
    Cancelled,
    AlreadyFinished,
    UnknownJob,
}

/// Flag the job as cancelled and report what was there.
///
/// Killing the process is the caller's job — this only records intent, which is
/// what closes the window between registering and spawning.
pub fn request_cancel(job_id: &str) -> CancelOutcome {
    let mut map = lock_registry();
    match map.get_mut(job_id) {
        None => CancelOutcome::UnknownJob,
        Some(entry) if entry.snapshot.status.is_terminal() => CancelOutcome::AlreadyFinished,
        Some(entry) => {
            entry.cancel_requested = true;
            CancelOutcome::Cancelled
        }
    }
}

/// Whether a job is claiming `path` right now. Test seam for the conflict rule.
#[cfg(test)]
fn destination_is_claimed(path: &std::path::Path) -> bool {
    lock_registry()
        .values()
        .any(|e| !e.snapshot.status.is_terminal() && e.destination.as_deref() == Some(path))
}

/// Forget everything. Tests only — the registry is process-global.
#[cfg(test)]
fn reset() {
    lock_registry().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-global, so these tests must not interleave.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        guard
    }

    #[test]
    fn a_registered_job_is_listed_before_it_runs() {
        let _guard = exclusive();
        register("save-1", "save", "alpine:3.19", Some(10), None).unwrap();

        let listed = list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, TransferStatus::Starting);
        assert_eq!(listed[0].total_estimate, Some(10));
    }

    /// The failure this whole module exists for: an outcome that lands before the
    /// caller even holds the job id must still be readable afterwards.
    #[test]
    fn an_outcome_survives_long_enough_to_be_reconciled() {
        let _guard = exclusive();
        register("save-2", "save", "alpine:3.19", None, None).unwrap();
        settle("save-2", TransferStatus::Failed, 0, Some("boom".into()));

        let listed = list();
        assert_eq!(listed.len(), 1, "a finished job must not vanish immediately");
        assert_eq!(listed[0].status, TransferStatus::Failed);
        assert_eq!(listed[0].error.as_deref(), Some("boom"));
    }

    #[test]
    fn two_jobs_cannot_write_the_same_destination() {
        let _guard = exclusive();
        let dest = PathBuf::from("/tmp/colimaui-registry-test/out.tar");

        register("save-3", "save", "a", None, Some(dest.clone())).unwrap();
        let err = register("save-4", "save", "b", None, Some(dest.clone()))
            .expect_err("the second job must be refused");
        assert!(err.contains("save-3"), "unexpected: {}", err);

        // Once the first finishes the path is free, even though its entry is still
        // readable for the retention window.
        settle("save-3", TransferStatus::Success, 1, None);
        assert!(!destination_is_claimed(&dest));
        register("save-4", "save", "b", None, Some(dest)).expect("the path is free now");
    }

    #[test]
    fn different_destinations_do_not_conflict() {
        let _guard = exclusive();
        register("save-5", "save", "a", None, Some("/tmp/a.tar".into())).unwrap();
        register("save-6", "save", "b", None, Some("/tmp/b.tar".into()))
            .expect("unrelated paths are independent");
    }

    #[test]
    fn cancelling_distinguishes_missing_from_finished() {
        let _guard = exclusive();
        assert_eq!(request_cancel("nope"), CancelOutcome::UnknownJob);

        register("cp-out-1", "cp-out", "/tmp/x", None, None).unwrap();
        assert_eq!(request_cancel("cp-out-1"), CancelOutcome::Cancelled);

        settle("cp-out-1", TransferStatus::Cancelled, 0, None);
        assert_eq!(request_cancel("cp-out-1"), CancelOutcome::AlreadyFinished);
    }

    /// A cancel arriving before the child exists must not be lost.
    #[test]
    fn a_cancel_during_startup_stops_the_job_from_running() {
        let _guard = exclusive();
        register("save-7", "save", "a", None, None).unwrap();
        assert_eq!(request_cancel("save-7"), CancelOutcome::Cancelled);

        assert!(
            !set_running("save-7"),
            "a job cancelled during startup must not proceed"
        );
    }

    #[test]
    fn a_job_nobody_cancelled_proceeds() {
        let _guard = exclusive();
        register("save-8", "save", "a", None, None).unwrap();
        assert!(set_running("save-8"));
        assert_eq!(list()[0].status, TransferStatus::Running);
    }
}
