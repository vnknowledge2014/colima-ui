//! Long-running CLI commands whose output must not sit in memory.
//!
//! `helpers::run_cmd` uses `Command::output()`, which buffers all of stdout into
//! RAM. That is fine for `docker version` and ruinous for `docker save` on a 2 GB
//! image. This module streams instead: stdout goes straight to a file descriptor
//! or is consumed line by line, so resident memory stays flat regardless of how
//! much data passes through.
//!
//! It also owns what a plain `Stdio::piped()` would not give us: cancelling a
//! command mid-flight, reporting progress, cleaning up half-written output, and
//! killing every child on the way out of the app.
//!
//! Environment setup and the "not installed" message are shared with `run_cmd`
//! via `helpers::build_cmd` / `helpers::describe_exec_error` — deliberately not
//! reimplemented here.
//!
//! **Path confinement is the caller's job.** This module writes wherever it is
//! told. See the base-directory policy documented on
//! `validation::assert_path_within`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::helpers::{build_cmd, describe_exec_error};

/// How often the wait loop reports progress and checks for cancellation.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// How long to wait for a killed command's stderr before giving up on it.
///
/// A grandchild that escaped the process-group kill still holds the pipe's write
/// end open, and reading it to EOF would then block for as long as that process
/// lives. Cancellation must not inherit somebody else's lifetime.
const STDERR_TIMEOUT: Duration = Duration::from_secs(2);

/// Where a command's stdout goes.
pub enum OutputSink {
    /// Straight to this file. Nothing passes through our address space, so this
    /// is the sink for multi-gigabyte payloads like `docker save`.
    ToFile(PathBuf),
    /// One message per line of stdout. For chatty-but-small output that the UI
    /// wants to display as it arrives.
    ByLine(Sender<String>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct StreamOutcome {
    /// Bytes written to the file, or bytes of stdout consumed, whichever applies.
    pub bytes: u64,
    /// True when the run ended because someone called [`cancel_stream`].
    pub cancelled: bool,
}

struct RunningCmd {
    child: Arc<Mutex<Child>>,
    cancelled: Arc<AtomicBool>,
}

/// Every stream currently in flight, so they can be cancelled by id and killed
/// wholesale when the app exits.
static RUNNING: LazyLock<Mutex<HashMap<String, RunningCmd>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Run `program` with output streamed to `sink`.
///
/// Blocks until the command finishes, is cancelled, or fails — call it from a
/// blocking context (`helpers::run_blocking`). `on_progress` receives the running
/// byte count roughly every [`POLL_INTERVAL`].
///
/// `job_id` must be unique among running streams; a collision is refused rather
/// than resolved. Note that this says nothing about *destinations* — this module
/// writes wherever it is told and two jobs may name one path. The scratch file is
/// therefore keyed by job id as well, so concurrent writers cannot interleave into
/// one file. (`commands::file_transfer` refuses that case a layer up, where it can
/// see the other job; nothing here depends on it having done so.)
///
/// A `ToFile` destination is only ever written by renaming a completed scratch file
/// onto it, so cancellation and non-zero exits publish nothing and leave whatever
/// was already at that path alone. A truncated tar is worse than no tar: it looks
/// loadable and is not.
pub fn run_cmd_streaming(
    job_id: &str,
    program: &str,
    args: &[&str],
    sink: OutputSink,
    on_progress: impl FnMut(u64),
) -> Result<StreamOutcome, String> {
    run_streaming(job_id, build_cmd(program, args), program, sink, on_progress)
}

/// Stream an already-built command.
///
/// Container-runtime callers must use this rather than [`run_cmd_streaming`]:
/// `commands::runtime::get_runtime_cmd` is what resolves docker vs podman vs the
/// `colima nerdctl` wrapper, along with the binary path and `DOCKER_HOST`.
/// Rebuilding that from a program name would work for docker and quietly fail for
/// everyone else.
///
/// `program_label` only names the command in error messages.
pub fn run_streaming(
    job_id: &str,
    cmd: Command,
    program_label: &str,
    sink: OutputSink,
    on_progress: impl FnMut(u64),
) -> Result<StreamOutcome, String> {
    match sink {
        OutputSink::ToFile(path) => {
            stream_to_file(job_id, cmd, program_label, &path, on_progress)
        }
        OutputSink::ByLine(tx) => stream_by_line(job_id, cmd, program_label, tx, on_progress),
    }
}

/// Spawn in a dedicated process group, with stdout wired to `stdout`.
///
/// The process group is what makes cancellation complete: `Child::kill` signals
/// only the direct child, so a command that forks helpers would leave them
/// running — and holding the stderr pipe open. Verified empirically:
/// `sh -c 'printf x; sleep 10'` survives a `Child::kill` for the full 10s.
fn spawn(mut cmd: Command, stdout: Stdio) -> Result<Child, std::io::Error> {
    cmd.stdout(stdout).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // 0 means "become your own group leader", so pgid == pid.
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Signal the child and everything it spawned.
fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // Negative pid targets the whole group. `process_group(0)` at spawn time
        // made the child its own leader, so this can never reach our own group.
        if pid > 0 {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    // Also signal the child directly: on non-unix, and as a fallback if the
    // group signal did not land.
    let _ = child.kill();
}

/// Ask a running stream to stop. Returns false if no such stream is running.
///
/// Marks the job cancelled *before* killing, so the waiting thread reports
/// cancellation rather than mistaking the kill for a command failure.
pub fn cancel_stream(job_id: &str) -> bool {
    let Ok(map) = RUNNING.lock() else {
        return false;
    };
    let Some(entry) = map.get(job_id) else {
        return false;
    };
    entry.cancelled.store(true, Ordering::SeqCst);
    if let Ok(mut child) = entry.child.lock() {
        kill_group(&mut child);
    }
    true
}

/// Kill every running stream. Called when the app is exiting: a `docker save`
/// started from a window that no longer exists has nothing left to report to.
///
/// Returns how many were killed.
pub fn kill_all_streams() -> usize {
    let Ok(map) = RUNNING.lock() else {
        return 0;
    };
    let mut killed = 0;
    for entry in map.values() {
        entry.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut child) = entry.child.lock() {
            kill_group(&mut child);
            killed += 1;
        }
    }
    killed
}

pub fn active_stream_count() -> usize {
    RUNNING.lock().map(|m| m.len()).unwrap_or(0)
}

/// Job ids cancelled before their child existed.
///
/// `cancel_stream` can only kill a process that is already registered, and there is
/// a real gap before that: the caller holds the job id as soon as the transfer is
/// accepted, but the child is spawned on a blocking task that may not have been
/// scheduled yet. A cancel landing in that gap used to be dropped, and the job ran
/// to completion after the user stopped it. [`arm_cancel`] records the intent here
/// instead, and [`register`] honours it the moment the child appears.
static PENDING_CANCEL: LazyLock<Mutex<std::collections::HashSet<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

/// Cancel a stream, or arm the cancel if its child does not exist yet.
///
/// Unlike [`cancel_stream`] this is for callers that already know the job is real —
/// otherwise a typo would leave an entry armed forever. `commands::file_transfer`
/// checks its own registry first.
///
/// Returns true when a running child was killed outright.
pub fn arm_cancel(job_id: &str) -> bool {
    if let Ok(mut pending) = PENDING_CANCEL.lock() {
        pending.insert(job_id.to_string());
    }
    cancel_stream(job_id)
}

/// Forget an armed cancel that was never consumed.
///
/// A job that finished on its own leaves its entry behind; without this the set
/// would grow for the life of the process and a later job reusing the id — which
/// cannot happen today, but ids are only a counter — would be cancelled at birth.
pub fn disarm_cancel(job_id: &str) {
    if let Ok(mut pending) = PENDING_CANCEL.lock() {
        pending.remove(job_id);
    }
}

/// Register a spawned child, refusing a duplicate id.
///
/// The returned flag starts out true when the job was cancelled while it was still
/// starting, so the wait loop stops it on its first tick rather than running a
/// command nobody wants any more.
fn register(job_id: &str, child: Child) -> Result<(Arc<Mutex<Child>>, Arc<AtomicBool>), String> {
    let armed = PENDING_CANCEL
        .lock()
        .map(|mut pending| pending.remove(job_id))
        .unwrap_or(false);

    let mut map = RUNNING
        .lock()
        .map_err(|_| "Stream registry is poisoned".to_string())?;
    if map.contains_key(job_id) {
        return Err(format!("A stream with id {} is already running", job_id));
    }
    let handle = Arc::new(Mutex::new(child));
    let cancelled = Arc::new(AtomicBool::new(armed));
    map.insert(
        job_id.to_string(),
        RunningCmd {
            child: handle.clone(),
            cancelled: cancelled.clone(),
        },
    );
    Ok((handle, cancelled))
}

fn unregister(job_id: &str) {
    if let Ok(mut map) = RUNNING.lock() {
        map.remove(job_id);
    }
}

/// Drain stderr on its own thread.
///
/// Two reasons this is a thread and not an inline read: the child blocks once it
/// has filled a pipe buffer's worth of stderr while we are still waiting on
/// stdout, and the result must be retrievable *with a deadline* — hence a channel
/// rather than a `JoinHandle`, which can only be joined unboundedly.
fn drain_stderr(child: &mut Child) -> Option<Receiver<String>> {
    let stderr = child.stderr.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    Some(rx)
}

/// Read the drained stderr, giving up after [`STDERR_TIMEOUT`].
///
/// Only worth calling on the failure path — that is the only time the text is
/// used, and waiting for it otherwise would make a successful or cancelled run
/// hostage to whatever still holds the pipe open.
fn collect_stderr(rx: Option<Receiver<String>>) -> String {
    rx.and_then(|rx| rx.recv_timeout(STDERR_TIMEOUT).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Sibling scratch file a streamed download is written to before it is published.
///
/// A sibling rather than a temp directory: the rename below is only atomic within
/// one filesystem, and `/tmp` frequently is not the same one as the folder the
/// user picked.
///
/// The job id is part of the name because two jobs can legitimately target one
/// destination — the registry dedups on job id, and `resolve_destination` cannot
/// see a conflict since the destination does not exist yet while the first job is
/// still writing. Keying only on the destination let both open the same scratch
/// file and interleave their bytes into it. Now each job writes its own and the
/// loser of the rename is simply overwritten by a complete archive.
///
/// Public so that tests observing a transfer in flight watch the file that is
/// actually growing, rather than re-deriving the convention and drifting from it.
pub fn partial_sink_path(path: &Path, job_id: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.part", sanitize_job_id(job_id)));
    path.with_file_name(name)
}

/// Keep a job id usable as part of a file name.
///
/// Ids are generated internally (`save-3`, `cp-out-12`) so this is belt and braces,
/// but the id reaches the filesystem here and a separator in one would place the
/// scratch file somewhere else entirely.
fn sanitize_job_id(job_id: &str) -> String {
    job_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

/// Run `cmd` with stdout redirected into `path`.
///
/// The bytes land in a `.part` sibling and are renamed onto `path` only after the
/// command exits successfully. Writing straight to `path` meant `File::create`
/// truncated whatever was already there, and every abort path then removed it — so
/// cancelling an overwrite destroyed the file the user still had. Nothing touches
/// `path` until there is a complete artefact to put there.
fn stream_to_file(
    job_id: &str,
    cmd: Command,
    program: &str,
    path: &Path,
    mut on_progress: impl FnMut(u64),
) -> Result<StreamOutcome, String> {
    let partial = partial_sink_path(path, job_id);
    let file = std::fs::File::create(&partial)
        .map_err(|e| format!("Cannot create {}: {}", partial.display(), e))?;

    let mut child = match spawn(cmd, Stdio::from(file)) {
        Ok(child) => child,
        Err(e) => {
            // We created the scratch file, so we remove it — a failed spawn must
            // not leave a zero-byte artefact behind.
            let _ = std::fs::remove_file(&partial);
            return Err(describe_exec_error(program, &e));
        }
    };

    let stderr = drain_stderr(&mut child);
    let (handle, cancelled) = match register(job_id, child) {
        Ok(pair) => pair,
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
    };

    // Cancelled while it was still starting. The flag alone stops nothing — the
    // wait below is waiting on the process, not on a boolean — so the kill that
    // `cancel_stream` could not deliver has to happen here.
    if cancelled.load(Ordering::SeqCst) {
        if let Ok(mut child) = handle.lock() {
            kill_group(&mut child);
        }
    }

    let status = wait_with_progress(&handle, || on_progress(file_len(&partial)));
    unregister(job_id);

    if cancelled.load(Ordering::SeqCst) {
        // stderr is deliberately not collected here: it is unused on this path,
        // and waiting for it would make cancellation as slow as the process that
        // holds the pipe.
        let _ = std::fs::remove_file(&partial);
        return Ok(StreamOutcome {
            bytes: 0,
            cancelled: true,
        });
    }

    let status = status.map_err(|e| format!("Failed to wait for {}: {}", program, e))?;
    if !status.success() {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("{} failed: {}", program, collect_stderr(stderr)));
    }

    std::fs::rename(&partial, path).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("Cannot write {}: {}", path.display(), e)
    })?;

    let bytes = file_len(path);
    on_progress(bytes);
    Ok(StreamOutcome {
        bytes,
        cancelled: false,
    })
}

fn stream_by_line(
    job_id: &str,
    cmd: Command,
    program: &str,
    tx: Sender<String>,
    mut on_progress: impl FnMut(u64),
) -> Result<StreamOutcome, String> {
    let mut child =
        spawn(cmd, Stdio::piped()).map_err(|e| describe_exec_error(program, &e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{}: stdout was not captured", program))?;
    let stderr = drain_stderr(&mut child);
    let (handle, cancelled) = register(job_id, child)?;

    // Cancelled during startup: deliver the kill `cancel_stream` could not, so the
    // read loop below ends on the resulting error rather than streaming a command
    // the user already stopped.
    if cancelled.load(Ordering::SeqCst) {
        if let Ok(mut child) = handle.lock() {
            kill_group(&mut child);
        }
    }

    let mut bytes: u64 = 0;
    let mut last_report = Instant::now();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => {
                bytes += n as u64;
                // A dropped receiver means nobody is listening any more; stop
                // the command instead of streaming into the void.
                if tx.send(line.trim_end().to_string()).is_err() {
                    if let Ok(mut child) = handle.lock() {
                        kill_group(&mut child);
                    }
                    break;
                }
                if last_report.elapsed() >= POLL_INTERVAL {
                    last_report = Instant::now();
                    on_progress(bytes);
                }
            }
            // A cancel kills the child, which surfaces here as a read error.
            Err(_) => break,
        }
    }

    let status = {
        let mut child = handle
            .lock()
            .map_err(|_| format!("{}: child handle is poisoned", program))?;
        child.wait()
    };
    unregister(job_id);

    if cancelled.load(Ordering::SeqCst) {
        // See the note on the same branch in `stream_to_file`: stderr is not
        // collected on a cancelled run.
        return Ok(StreamOutcome {
            bytes,
            cancelled: true,
        });
    }

    let status = status.map_err(|e| format!("Failed to wait for {}: {}", program, e))?;
    if !status.success() {
        return Err(format!("{} failed: {}", program, collect_stderr(stderr)));
    }

    on_progress(bytes);
    Ok(StreamOutcome {
        bytes,
        cancelled: false,
    })
}

/// Wait for the child, running `tick` between polls.
///
/// The lock is held only for `try_wait` and released before sleeping, so
/// `cancel_stream` is never blocked waiting for this loop.
fn wait_with_progress(
    handle: &Arc<Mutex<Child>>,
    mut tick: impl FnMut(),
) -> std::io::Result<std::process::ExitStatus> {
    loop {
        {
            let mut child = handle
                .lock()
                .map_err(|_| std::io::Error::other("child handle is poisoned"))?;
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
        }
        tick();
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// The registry is process-global, and so is `kill_all_streams` — by design,
    /// since app exit must reap everything. That makes these tests mutually
    /// exclusive: cargo runs them on parallel threads in one process, where one
    /// test's `kill_all_streams` would cancel another's running command. Every
    /// test in this module takes this guard first.
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    /// Take the serialization guard, ignoring poisoning from an earlier failure —
    /// one failing test should not cascade into false failures in the rest.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn job(name: &str) -> String {
        format!("test-{}-{:?}", name, std::thread::current().id())
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("colimaui-stream-{}-{:?}", name, std::thread::current().id()))
    }

    #[test]
    fn writes_stdout_to_file_and_reports_progress() {
        let _guard = exclusive();
        let path = temp_path("to-file");
        let mut reports = Vec::new();
        let outcome = run_cmd_streaming(
            &job("to-file"),
            "sh",
            &["-c", "printf 'hello world'"],
            OutputSink::ToFile(path.clone()),
            |b| reports.push(b),
        )
        .expect("command should succeed");

        assert_eq!(outcome.bytes, 11);
        assert!(!outcome.cancelled);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
        assert_eq!(reports.last(), Some(&11));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn yields_stdout_line_by_line() {
        let _guard = exclusive();
        let (tx, rx) = mpsc::channel();
        let outcome = run_cmd_streaming(
            &job("by-line"),
            "sh",
            &["-c", "printf 'a\\nb\\nc\\n'"],
            OutputSink::ByLine(tx),
            |_| {},
        )
        .expect("command should succeed");

        let lines: Vec<String> = rx.iter().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
        assert_eq!(outcome.bytes, 6);
    }

    #[test]
    fn failed_command_reports_stderr_and_removes_partial_file() {
        let _guard = exclusive();
        let path = temp_path("failing");
        let err = run_cmd_streaming(
            &job("failing"),
            "sh",
            &["-c", "printf 'partial'; echo 'went wrong' >&2; exit 3"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect_err("non-zero exit must be an error");

        assert!(err.contains("went wrong"), "stderr should reach the caller: {}", err);
        // A truncated archive that looks complete is worse than none at all.
        assert!(!path.exists(), "partial output must not survive a failure");
    }

    #[test]
    fn cancel_kills_the_child_and_discards_partial_output() {
        let _guard = exclusive();
        let path = temp_path("cancel");
        let id = job("cancel");
        let id_for_thread = id.clone();

        // Cancel from another thread while the command is still running.
        let canceller = std::thread::spawn(move || {
            for _ in 0..100 {
                if cancel_stream(&id_for_thread) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        });

        let outcome = run_cmd_streaming(
            &id,
            "sh",
            &["-c", "printf 'started'; sleep 30"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect("cancellation is not a failure");

        assert!(canceller.join().unwrap(), "cancel_stream should have found the job");
        assert!(outcome.cancelled);
        assert!(!path.exists(), "cancelled output must be cleaned up");
        assert!(
            !partial_sink_path(&path, &id).exists(),
            "the scratch file must go too"
        );
        // No orphan: the registry entry is gone and the child was reaped.
        assert!(!cancel_stream(&id), "job should no longer be registered");
    }

    /// Cancelling an overwrite must leave the file the user already had.
    ///
    /// Output used to be written straight to the destination, so `File::create`
    /// truncated it on the first byte and the abort path then deleted it. Cancelling
    /// an export over an existing archive therefore destroyed that archive — the
    /// user lost a file by changing their mind. Nothing touches the destination now
    /// until there is a complete artefact to rename onto it.
    #[test]
    fn cancelling_does_not_destroy_an_existing_destination() {
        let _guard = exclusive();
        let path = temp_path("cancel-keeps-existing");
        std::fs::write(&path, b"the file the user already had").unwrap();

        let id = job("cancel-keeps-existing");
        let id_for_thread = id.clone();
        let canceller = std::thread::spawn(move || {
            for _ in 0..100 {
                if cancel_stream(&id_for_thread) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        });

        let outcome = run_cmd_streaming(
            &id,
            "sh",
            &["-c", "printf 'replacement'; sleep 30"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect("cancellation is not a failure");

        assert!(canceller.join().unwrap(), "cancel_stream should have found the job");
        assert!(outcome.cancelled);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"the file the user already had",
            "cancelling must not touch the destination"
        );
        assert!(!partial_sink_path(&path, &id).exists());

        let _ = std::fs::remove_file(&path);
    }

    /// Two jobs aimed at one destination must not write into the same scratch file.
    ///
    /// The registry dedups on job id, and the destination does not exist while the
    /// first job is running — so nothing upstream refuses this. Keying the scratch
    /// file on the destination alone let both processes open it and interleave
    /// their bytes into a single corrupt archive.
    #[test]
    fn concurrent_jobs_to_one_destination_use_separate_scratch_files() {
        let path = temp_path("shared-destination");
        let a = partial_sink_path(&path, "save-1");
        let b = partial_sink_path(&path, "save-2");

        assert_ne!(a, b, "two jobs must not share a scratch file");
        assert_eq!(a.parent(), path.parent(), "scratch must stay a sibling");
    }

    /// A job id cannot place the scratch file outside the destination's folder.
    #[test]
    fn a_separator_in_a_job_id_cannot_escape_the_folder() {
        let path = temp_path("escape-attempt");
        let scratch = partial_sink_path(&path, "../../etc/passwd");

        assert_eq!(scratch.parent(), path.parent());
        assert!(!scratch.to_string_lossy().contains(".."));
    }

    /// A failing command must leave an existing destination alone as well.
    #[test]
    fn a_failed_command_does_not_destroy_an_existing_destination() {
        let _guard = exclusive();
        let path = temp_path("fail-keeps-existing");
        std::fs::write(&path, b"previous archive").unwrap();

        let id = job("fail-keeps-existing");
        let err = run_cmd_streaming(
            &id,
            "sh",
            &["-c", "printf 'partial'; exit 3"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect_err("a non-zero exit is a failure");
        assert!(!err.is_empty());

        assert_eq!(std::fs::read(&path).unwrap(), b"previous archive");
        assert!(!partial_sink_path(&path, &id).exists());

        let _ = std::fs::remove_file(&path);
    }

    /// Cancelling must return promptly even when the command forked a helper.
    ///
    /// `sh -c 'printf x; sleep 30'` cannot be exec'd away, so `sh` forks `sleep`,
    /// which inherits the stderr pipe. Signalling only the direct child left the
    /// grandchild alive holding that pipe, and reading stderr to EOF then blocked
    /// for the grandchild's full 30 seconds — cancellation appeared to hang.
    #[test]
    fn cancel_returns_promptly_even_when_the_command_forked_a_child() {
        let _guard = exclusive();
        let path = temp_path("cancel-fast");
        let id = job("cancel-fast");
        let id_for_thread = id.clone();

        let canceller = std::thread::spawn(move || {
            for _ in 0..100 {
                if cancel_stream(&id_for_thread) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        });

        let started = Instant::now();
        let outcome = run_cmd_streaming(
            &id,
            "sh",
            // stderr write plus a long sleep: two commands, so no exec.
            &["-c", "printf 'x' >&2; sleep 30"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect("cancellation is not a failure");
        let elapsed = started.elapsed();

        assert!(canceller.join().unwrap());
        assert!(outcome.cancelled);
        assert!(
            elapsed < Duration::from_secs(5),
            "cancel took {:?}; it is waiting on a process it does not own",
            elapsed
        );
        assert!(!path.exists());
    }

    /// The grandchild itself must die, not merely stop blocking us.
    #[test]
    fn cancel_leaves_no_orphan_process() {
        let _guard = exclusive();
        let path = temp_path("orphan");
        let id = job("orphan");
        let id_for_thread = id.clone();
        // A marker unique to this run, so `pgrep` cannot match anything else.
        let marker = format!("colimaui-orphan-probe-{}", std::process::id());
        let script = format!("printf 'x'; sleep 47 # {}", marker);

        let canceller = std::thread::spawn(move || {
            for _ in 0..100 {
                if cancel_stream(&id_for_thread) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });

        let outcome = run_cmd_streaming(
            &id,
            "sh",
            &["-c", &script],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect("cancellation is not a failure");
        canceller.join().unwrap();
        assert!(outcome.cancelled);

        // Give the signal a moment to be reaped, then look for survivors.
        std::thread::sleep(Duration::from_millis(300));
        let survivors = std::process::Command::new("pgrep")
            .args(["-f", &marker])
            .output()
            .expect("pgrep should be available");
        let found = String::from_utf8_lossy(&survivors.stdout).trim().to_string();
        assert!(
            found.is_empty(),
            "orphan process(es) survived cancellation: pids {}",
            found
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn refuses_a_duplicate_job_id() {
        let _guard = exclusive();
        let id = job("dup");
        let path = temp_path("dup");
        let id_inner = id.clone();
        let path_inner = temp_path("dup-inner");

        // Hold one job open, then try to start a second under the same id.
        let blocker = std::thread::spawn(move || {
            run_cmd_streaming(
                &id_inner,
                "sh",
                &["-c", "sleep 5"],
                OutputSink::ToFile(path_inner),
                |_| {},
            )
        });

        let mut err = None;
        for _ in 0..100 {
            if active_stream_count() > 0 {
                err = run_cmd_streaming(
                    &id,
                    "sh",
                    &["-c", "printf x"],
                    OutputSink::ToFile(path.clone()),
                    |_| {},
                )
                .err();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        cancel_stream(&id);
        let _ = blocker.join();

        let err = err.expect("second start with the same id must fail");
        assert!(err.contains("already running"), "unexpected error: {}", err);
        // The refusal must not clobber the running job's destination.
        assert!(!path.exists());
    }

    #[test]
    fn kill_all_streams_reaps_everything() {
        let _guard = exclusive();
        let id = job("kill-all");
        let id_inner = id.clone();
        let path = temp_path("kill-all");

        let runner = std::thread::spawn(move || {
            run_cmd_streaming(
                &id_inner,
                "sh",
                &["-c", "sleep 30"],
                OutputSink::ToFile(path),
                |_| {},
            )
        });

        let mut killed = 0;
        for _ in 0..100 {
            if active_stream_count() > 0 {
                killed = kill_all_streams();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let outcome = runner.join().unwrap().expect("kill is a cancellation");
        assert!(killed >= 1);
        assert!(outcome.cancelled);
        assert!(!cancel_stream(&id));
    }

    #[test]
    fn missing_program_reports_the_same_hint_as_run_cmd() {
        let _guard = exclusive();
        let err = run_cmd_streaming(
            &job("missing"),
            "colimaui-no-such-binary",
            &[],
            OutputSink::ToFile(temp_path("missing")),
            |_| {},
        )
        .expect_err("a missing binary must fail");

        assert!(err.contains("is not installed"), "unexpected error: {}", err);
        // The destination file we opened before spawning must not be left behind.
        assert!(!temp_path("missing").exists());
    }

    /// The reason this module exists: resident memory must not track the size of
    /// the payload. `Command::output()` would hold all 256 MB at once.
    #[test]
    fn resident_memory_does_not_track_payload_size() {
        let _guard = exclusive();
        let path = temp_path("rss");
        let before = resident_kb().expect("cannot read RSS on this platform");

        let outcome = run_cmd_streaming(
            &job("rss"),
            "sh",
            &["-c", "dd if=/dev/zero bs=1048576 count=256 2>/dev/null"],
            OutputSink::ToFile(path.clone()),
            |_| {},
        )
        .expect("command should succeed");

        let after = resident_kb().expect("cannot read RSS on this platform");
        let _ = std::fs::remove_file(&path);

        assert_eq!(outcome.bytes, 256 * 1024 * 1024);
        let growth_kb = after.saturating_sub(before);
        assert!(
            growth_kb < 32 * 1024,
            "RSS grew {} KB while streaming 256 MB — output is being buffered",
            growth_kb
        );
    }

    fn resident_kb() -> Option<u64> {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
}
