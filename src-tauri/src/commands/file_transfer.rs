//! Copying files in and out of containers, and moving images as TAR archives.
//!
//! A module of its own rather than an addition to `containers.rs` / `images.rs`:
//! it cuts across both, and unlike everything in those files these operations
//! have a lifecycle — they run in the background, report progress and can be
//! cancelled.
//!
//! Every command here streams via `streaming_cmd` and never `run_cmd`: a
//! `docker save` of a large image through `Command::output()` would sit in RAM in
//! its entirety.
//!
//! ## Where the safety comes from
//!
//! Arguments are passed with `Command::args`, never interpolated into a shell, so
//! a container path containing shell metacharacters is inert. `contains_shell_injection`
//! is deliberately *not* used here — it guards user-typed exec commands and is not
//! a path check.
//!
//! Host paths that get *written* are confined with `validation::assert_path_within`
//! against the directory the user picked in a dialog. That confinement is only
//! meaningful because the base arrives separately from the file name: checking a
//! path against its own parent would pass by construction.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

use crate::commands::activity;
use crate::error::ColimaError;
use crate::streaming_cmd::{run_streaming, OutputSink};
use crate::transfer_registry as registry;

static JOB_SEQ: AtomicU64 = AtomicU64::new(1);

/// Floor for starting a copy out of a container.
///
/// Unlike an image export there is no size to estimate — the source lives inside the
/// container and the runtime reports no total — so this only refuses a stream that
/// has nowhere to go at all.
const MIN_COPY_OUT_FREE_BYTES: u64 = 256 * 1024 * 1024;

fn next_job_id(kind: &str) -> String {
    format!("{}-{}", kind, JOB_SEQ.fetch_add(1, Ordering::Relaxed))
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransferStarted {
    pub job_id: String,
    /// Best-effort byte total, when one can be had at all. Docker reports no total
    /// for these operations, so this comes from image metadata — it is labelled an
    /// estimate everywhere it surfaces because it is one.
    pub total_estimate: Option<u64>,
}

/// Emit on both channels.
///
/// The desktop app listens to Tauri events; browser mode listens to SSE. A job
/// started over IPC has an `AppHandle`, one started over HTTP does not, and
/// publishing to SSE unconditionally costs nothing when nobody is subscribed.
fn emit(app: Option<&AppHandle>, event: &str, payload: serde_json::Value) {
    crate::sse::publish_sse_event(event, &payload);
    if let Some(app) = app {
        let _ = app.emit(event, payload);
    }
}

fn progress_event(job_id: &str, bytes: u64, total: Option<u64>, message: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "jobId": job_id,
        "bytes": bytes,
        "totalEstimate": total,
        "message": message,
    })
}

// ===== Validation =====

/// Reject an argument that would be read as a flag rather than a value.
///
/// These all go to the runtime positionally (`docker save <ref>`, `docker cp <src>`),
/// so a value starting with `-` changes the command's meaning.
fn reject_flag_like(label: &str, value: &str) -> Result<(), String> {
    if value.starts_with('-') {
        return Err(format!("{} must not start with '-': {:?}", label, value));
    }
    Ok(())
}

/// Reject empty and control-character arguments.
///
/// Control characters cannot cause injection here — nothing goes through a shell —
/// but they produce unreadable errors and corrupt log lines, and no legitimate
/// image reference or path contains them.
fn require_plain(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{} must not be empty", label));
    }
    if value.chars().any(|c| c.is_control()) {
        return Err(format!("{} contains control characters", label));
    }
    reject_flag_like(label, value)
}

/// Reject a container id that would be read as a flag.
///
/// `is_valid_container_id` permits `-` anywhere in the id, first position included,
/// because that is a legal character in a container *name*. Positionally these ids
/// are handed straight to the runtime, so one starting with `-` changes what the
/// command means. The `--` terminator below is the real defence; this keeps the
/// error message about the id rather than about argument parsing.
fn require_container_ref(container_id: &str) -> Result<(), String> {
    if !crate::validation::is_valid_container_id(container_id) {
        return Err(format!("Invalid container id: {:?}", container_id));
    }
    reject_flag_like("Container id", container_id)
}

/// Require a name that describes what these commands actually produce.
///
/// `docker save` and `docker cp <src> -` both write an **uncompressed** TAR. A name
/// ending in `.tar.gz`/`.tgz`/`.zip` would label the file as something it is not, so
/// those are refused rather than quietly honoured; a name with no extension at all
/// leaves a file the OS cannot classify and `docker load` cannot be pointed at
/// without guessing.
fn require_archive_name(file_name: &str) -> Result<(), String> {
    let lower = file_name.to_ascii_lowercase();
    for misleading in [".tar.gz", ".tgz", ".tar.bz2", ".tar.xz", ".zip", ".gz", ".bz2", ".xz"] {
        if lower.ends_with(misleading) {
            return Err(format!(
                "This writes an uncompressed archive, so {:?} would describe it wrongly. Use a name ending in .tar.",
                file_name
            ));
        }
    }
    if !lower.ends_with(".tar") {
        return Err(format!(
            "File name must end in .tar: {:?}",
            file_name
        ));
    }
    Ok(())
}

/// Reject a file that is plainly not the uncompressed TAR `docker load` expects.
///
/// This catches a mistaken pick — a `.zip`, a disk image, a log — before the runtime
/// is started, so the user gets a sentence about the file they chose instead of
/// whatever the runtime prints. It is **not** a security control: the file can change
/// between this read and the runtime's, and the last branch deliberately lets an
/// unrecognised-but-plausible archive through rather than blocking a valid one.
fn sniff_tar(path: &Path) -> Result<(), String> {
    use std::io::Read;

    // One TAR block. `read_to_end` on a capped reader rather than a single `read`,
    // which is free to return fewer bytes than are available and would make the
    // checks below depend on how the file happened to be buffered.
    let mut head = Vec::with_capacity(512);
    std::fs::File::open(path)
        .and_then(|f| f.take(512).read_to_end(&mut head))
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    if head.len() >= 2 && head[0] == 0x1f && head[1] == 0x8b {
        return Err(format!(
            "{} is gzip-compressed. Decompress it first — this imports an uncompressed archive.",
            path.display()
        ));
    }
    if head.len() >= 4 && &head[0..4] == b"PK\x03\x04" {
        return Err(format!("{} is a ZIP archive, not a TAR archive.", path.display()));
    }
    // POSIX ustar and GNU tar both carry this magic at offset 257.
    if head.len() >= 262 && &head[257..262] == b"ustar" {
        return Ok(());
    }
    // Pre-POSIX tars have no magic at all. A full block behind a `.tar` name is
    // accepted rather than rejecting an archive the runtime would have read fine.
    if head.len() >= 512 && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tar")) {
        return Ok(());
    }
    Err(format!(
        "{} does not look like a TAR archive produced by `docker save`.",
        path.display()
    ))
}

/// Resolve `dir` + `file_name` into a destination, refusing anything that escapes
/// `dir` or would silently overwrite.
fn resolve_destination(
    dir: &str,
    file_name: &str,
    overwrite: bool,
) -> Result<PathBuf, String> {
    require_plain("Destination folder", dir)?;
    require_plain("File name", file_name)?;

    let base = Path::new(dir);
    if !base.is_dir() {
        return Err(format!("Destination folder does not exist: {}", dir));
    }

    let candidate = base.join(file_name);
    // The base is the folder the user chose in the dialog, not the candidate's own
    // parent — that is what makes `../` in the file name a rejection instead of a
    // tautology.
    crate::validation::assert_path_within(base, &candidate)?;

    if candidate.exists() && !overwrite {
        return Err(format!(
            "{} already exists. Choose another name or confirm overwriting.",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn require_existing_file(label: &str, path: &str) -> Result<PathBuf, String> {
    require_plain(label, path)?;
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(format!("{} is not a file: {}", label, path));
    }
    Ok(p)
}

// ===== Disk space =====

/// Free bytes available on the filesystem holding `dir`.
///
/// Returns `None` when the platform call fails; callers treat that as "unknown"
/// and proceed rather than refusing a transfer over a missing statistic.
fn available_bytes(dir: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(dir.as_os_str().as_bytes()).ok()?;
        // SAFETY: `c_path` is a valid NUL-terminated string for the duration of
        // the call, and `stat` is fully initialised by statvfs on success.
        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c_path.as_ptr(), &mut stat) != 0 {
                return None;
            }
            Some((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        None
    }
}

/// Total reported size of the given images, used as the export estimate.
///
/// `docker save` writes uncompressed layers, so this is close but not exact —
/// which is why it never becomes a hard total, only an estimate and a disk check.
fn estimate_image_bytes(images: &[String]) -> Option<u64> {
    let mut cmd = crate::commands::runtime::get_runtime_cmd();
    cmd.args(["image", "inspect", "--format", "{{.Size}}"]);
    cmd.args(images);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let total: u64 = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u64>().ok())
        .sum();
    (total > 0).then_some(total)
}

// ===== Job runner =====

/// Spawn the streaming command on a blocking thread and report its outcome.
///
/// `sink_path` is the archive the command's stdout is written to, when it produces
/// one. Every job that writes something on this side now writes an archive that way,
/// so progress is the sink's growth and cleanup belongs entirely to `streaming_cmd`.
/// There used to be two further paths here — a directory to watch and a file to
/// delete on abort — for the days when `docker cp` created its own destination; that
/// destination could be a directory, which made the size meaningless and the
/// deletion a silent no-op.
/// What a background job is doing, in the terms the activity log records.
///
/// Carried into the spawned task so the row can be written where the outcome is
/// known. The fields are captured at `start_*` time because that is where the
/// names still exist — by the time the job ends the image may have been
/// exported and forgotten, and the log is the only place its name survives.
struct TransferSubject {
    /// `save`, `load`, `copy-in`, `copy-out`.
    verb: &'static str,
    /// `image`, `archive`, `container`.
    target_kind: &'static str,
    target: String,
    target_name: String,
}

impl TransferSubject {
    fn record(&self, outcome: activity::ActivityOutcome, detail: String, duration_ms: i64) {
        let mut entry = activity::ActivityEntry::new(
            activity::ActivityKind::Task,
            self.verb,
            self.target_kind,
            &self.target,
        )
        .named(&self.target_name)
        .detail(detail)
        .took(duration_ms);
        // Set directly: `outcome_of` reads a `Result`, and a cancellation is
        // neither arm of one.
        entry.outcome = outcome;
        activity::record(entry);
    }
}

/// `subject` describes what the job is doing, for the activity log. The row is
/// written here rather than in the four `start_*` functions because a transfer
/// is only worth recording once it has an ending: those return a job id within
/// milliseconds, long before there is an outcome or a duration, and a row
/// saying "started" that never gains a conclusion is noise.
fn spawn_job(
    app: Option<AppHandle>,
    job_id: String,
    mut cmd: std::process::Command,
    args: Vec<String>,
    sink_path: Option<PathBuf>,
    total_estimate: Option<u64>,
    subject: TransferSubject,
) {
    cmd.args(&args);
    tokio::task::spawn_blocking(move || {
        let app_ref = app.as_ref();
        let id = job_id.clone();
        let started = std::time::Instant::now();
        // Settles the entry if this task ends without reaching a terminal branch.
        // An entry left at `Running` would hold its destination claim forever.
        let guard = registry::JobGuard::new(&job_id);

        // A cancel can arrive between registration and this point — the caller has
        // the job id as soon as `start_*` returns, which is before this task is
        // scheduled. Without this check that cancel would be dropped and the
        // transfer would run to completion after the user stopped it.
        if !registry::set_running(&job_id) {
            registry::settle(&job_id, registry::TransferStatus::Cancelled, 0, None);
            crate::streaming_cmd::disarm_cancel(&job_id);
            guard.done();
            emit(
                app_ref,
                "transfer.done",
                serde_json::json!({ "jobId": job_id, "bytes": 0, "cancelled": true }),
            );
            // Cancelled before the command was spawned. Still recorded: the
            // user asked for something and it did not happen, which is exactly
            // what they come back looking for.
            subject.record(
                activity::ActivityOutcome::Cancelled,
                "cancelled before it started".to_string(),
                started.elapsed().as_millis() as i64,
            );
            return;
        }

        // `docker` prints no percentage for any of these, so the bytes written to
        // the sink are the only honest measure available.
        let emit_progress = |bytes: u64| {
            registry::update_bytes(&id, bytes);
            emit(
                app_ref,
                "transfer.progress",
                progress_event(&id, bytes, total_estimate, None),
            );
        };

        let (sink, lines) = match sink_path {
            Some(path) => (OutputSink::ToFile(path), None),
            None => {
                let (tx, rx) = std::sync::mpsc::channel();
                (OutputSink::ByLine(tx), Some(rx))
            }
        };

        // Forward the runtime's own progress lines (`docker load` emits them)
        // while the command runs.
        let line_pump = lines.map(|rx| {
            let app = app.clone();
            let id = job_id.clone();
            std::thread::spawn(move || {
                for line in rx {
                    if line.trim().is_empty() {
                        continue;
                    }
                    emit(
                        app.as_ref(),
                        "transfer.progress",
                        progress_event(&id, 0, None, Some(&line)),
                    );
                }
            })
        });

        let result = run_streaming(&job_id, cmd, "docker", sink, emit_progress);
        if let Some(pump) = line_pump {
            let _ = pump.join();
        }

        // `run_streaming` has returned, so a successful destination has already been
        // renamed into place. Settling here rather than when the child was reaped is
        // what lets a client treat `Success` as "the file is there".
        match result {
            Ok(outcome) if outcome.cancelled => {
                registry::settle(&job_id, registry::TransferStatus::Cancelled, 0, None);
                // A cancellation is not a failure: the user asked for it.
                emit(
                    app_ref,
                    "transfer.done",
                    serde_json::json!({ "jobId": job_id, "bytes": 0, "cancelled": true }),
                );
                subject.record(
                    activity::ActivityOutcome::Cancelled,
                    "cancelled part-way".to_string(),
                    started.elapsed().as_millis() as i64,
                );
            }
            Ok(outcome) => {
                registry::settle(
                    &job_id,
                    registry::TransferStatus::Success,
                    outcome.bytes,
                    None,
                );
                emit(
                    app_ref,
                    "transfer.done",
                    serde_json::json!({ "jobId": job_id, "bytes": outcome.bytes, "cancelled": false }),
                );
                subject.record(
                    activity::ActivityOutcome::Ok,
                    format!("{} transferred", human_bytes(outcome.bytes)),
                    started.elapsed().as_millis() as i64,
                );
            }
            Err(e) => {
                // Redact at the source, not at the display: this text leaves the
                // process on two transports, and there is no single downstream
                // place that covers both.
                //
                // Note what this does *not* do. `redact` masks the account segment
                // of a home directory and known secret shapes; it does not remove
                // arbitrary absolute paths, so `Cannot create /Volumes/T7/img.tar`
                // survives intact. That is acceptable for a message shown to the
                // user who chose that path, and is why the OS notification in a
                // later phase must not carry this field.
                let safe = crate::redact::redact(&e);
                registry::settle(
                    &job_id,
                    registry::TransferStatus::Failed,
                    0,
                    Some(safe.clone()),
                );
                emit(
                    app_ref,
                    "transfer.failed",
                    serde_json::json!({ "jobId": job_id, "error": safe }),
                );
                // `safe` is already redacted; `record` redacts again, which is
                // cheap and keeps this call site from having to be trusted.
                subject.record(
                    activity::ActivityOutcome::Failed,
                    safe,
                    started.elapsed().as_millis() as i64,
                );
            }
        }
        // Every branch above settled the entry.
        crate::streaming_cmd::disarm_cancel(&job_id);
        guard.done();
    });
}

// ===== Commands =====

/// Export images to a TAR archive.
///
/// stdout is redirected into the archive rather than using `docker save -o`, so
/// the bytes never enter this process and the file's growth doubles as progress.
#[tauri::command]
pub async fn image_save(
    app: AppHandle,
    images: Vec<String>,
    dest_dir: String,
    file_name: String,
    overwrite: bool,
) -> Result<TransferStarted, ColimaError> {
    Ok(start_image_save(Some(app), images, dest_dir, file_name, overwrite)?)
}

pub fn start_image_save(
    app: Option<AppHandle>,
    images: Vec<String>,
    dest_dir: String,
    file_name: String,
    overwrite: bool,
) -> Result<TransferStarted, String> {
    if images.is_empty() {
        return Err("Select at least one image to export".to_string());
    }
    for image in &images {
        require_plain("Image reference", image)?;
    }
    require_archive_name(&file_name)?;
    let dest = resolve_destination(&dest_dir, &file_name, overwrite)?;

    let total_estimate = estimate_image_bytes(&images);
    // Refuse rather than fill the disk. The estimate can be off, so the message
    // reports both numbers and lets the user pick another folder.
    if let (Some(needed), Some(free)) = (total_estimate, available_bytes(Path::new(&dest_dir))) {
        if free < needed {
            return Err(format!(
                "Not enough space in {}: about {} needed, {} free",
                dest_dir,
                human_bytes(needed),
                human_bytes(free)
            ));
        }
    }

    let job_id = next_job_id("save");
    // Registered before the spawn below: `spawn_blocking` can fail and emit a
    // terminal event before this function has even returned the id.
    registry::register(
        &job_id,
        "save",
        &images.join(", "),
        total_estimate,
        Some(dest.clone()),
    )?;
    // `--` so an image reference that starts with `-` is read as a value. The
    // reference is already checked, but the terminator is what makes that check
    // unnecessary rather than load-bearing.
    let mut args = vec!["save".to_string(), "--".to_string()];
    // Captured before `args` takes ownership: the log needs the names, and the
    // exported image may be gone by the time anyone reads the row.
    let exported = images.join(", ");
    args.extend(images);
    spawn_job(
        app,
        job_id.clone(),
        crate::commands::runtime::get_runtime_cmd(),
        args,
        Some(dest),
        total_estimate,
        TransferSubject {
            verb: "save",
            target_kind: "image",
            target: exported.clone(),
            target_name: exported,
        },
    );
    Ok(TransferStarted {
        job_id,
        total_estimate,
    })
}

/// Import images from a TAR archive produced by `docker save`.
#[tauri::command]
pub async fn image_load(app: AppHandle, tar_path: String) -> Result<TransferStarted, ColimaError> {
    Ok(start_image_load(Some(app), tar_path)?)
}

pub fn start_image_load(
    app: Option<AppHandle>,
    tar_path: String,
) -> Result<TransferStarted, String> {
    let path = require_existing_file("Archive", &tar_path)?;
    sniff_tar(&path)?;

    let job_id = next_job_id("load");
    registry::register(
        &job_id,
        "load",
        &path.file_name().unwrap_or_default().to_string_lossy(),
        None,
        None,
    )?;
    spawn_job(
        app,
        job_id.clone(),
        crate::commands::runtime::get_runtime_cmd(),
        vec![
            "load".to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ],
        // `docker load` reports progress on stdout (ByLine, not ToFile) and writes
        // no artefact on this side.
        None,
        None,
        TransferSubject {
            verb: "load",
            target_kind: "archive",
            target: path.to_string_lossy().to_string(),
            target_name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        },
    );
    Ok(TransferStarted {
        job_id,
        total_estimate: None,
    })
}

/// Copy a host file into a container.
///
/// The host path is only read, so it is not confined to a base directory — see the
/// policy table on `validation::assert_path_within`.
#[tauri::command]
pub async fn copy_to_container(
    app: AppHandle,
    container_id: String,
    host_path: String,
    container_path: String,
) -> Result<TransferStarted, ColimaError> {
    Ok(start_copy_to_container(
        Some(app),
        container_id,
        host_path,
        container_path,
    )?)
}

pub fn start_copy_to_container(
    app: Option<AppHandle>,
    container_id: String,
    host_path: String,
    container_path: String,
) -> Result<TransferStarted, String> {
    require_container_ref(&container_id)?;
    let source = require_existing_file("Source file", &host_path)?;
    require_plain("Container path", &container_path)?;

    let job_id = next_job_id("cp-in");
    registry::register(&job_id, "cp-in", &container_path, None, None)?;
    spawn_job(
        app,
        job_id.clone(),
        crate::commands::runtime::get_runtime_cmd(),
        vec![
            "cp".to_string(),
            "--".to_string(),
            source.to_string_lossy().to_string(),
            format!("{}:{}", container_id, container_path),
        ],
        // Copying *into* a container writes nothing on this side.
        None,
        None,
        TransferSubject {
            verb: "copy-in",
            target_kind: "container",
            target: container_id.clone(),
            target_name: container_path.clone(),
        },
    );
    Ok(TransferStarted {
        job_id,
        total_estimate: None,
    })
}

/// Copy a file out of a container into a user-chosen folder.
#[tauri::command]
pub async fn copy_from_container(
    app: AppHandle,
    container_id: String,
    container_path: String,
    dest_dir: String,
    file_name: String,
    overwrite: bool,
) -> Result<TransferStarted, ColimaError> {
    Ok(start_copy_from_container(
        Some(app),
        container_id,
        container_path,
        dest_dir,
        file_name,
        overwrite,
    )?)
}

pub fn start_copy_from_container(
    app: Option<AppHandle>,
    container_id: String,
    container_path: String,
    dest_dir: String,
    file_name: String,
    overwrite: bool,
) -> Result<TransferStarted, String> {
    require_container_ref(&container_id)?;
    require_plain("Container path", &container_path)?;
    require_archive_name(&file_name)?;
    let dest = resolve_destination(&dest_dir, &file_name, overwrite)?;

    // No total is knowable — the runtime reports none and the source is inside the
    // container — so this is a floor rather than a fit: refuse to start a stream
    // that has nowhere to go. Running out mid-copy still fails the job, but the
    // partial file is discarded by the sink rather than left behind.
    if let Some(free) = available_bytes(Path::new(&dest_dir)) {
        if free < MIN_COPY_OUT_FREE_BYTES {
            return Err(format!(
                "Not enough space in {}: {} free, at least {} needed",
                dest_dir,
                human_bytes(free),
                human_bytes(MIN_COPY_OUT_FREE_BYTES)
            ));
        }
    }

    let job_id = next_job_id("cp-out");
    registry::register(&job_id, "cp-out", &container_path, None, Some(dest.clone()))?;
    spawn_job(
        app,
        job_id.clone(),
        crate::commands::runtime::get_runtime_cmd(),
        vec![
            "cp".to_string(),
            "--".to_string(),
            format!("{}:{}", container_id, container_path),
            // `-` writes a TAR stream to stdout. Naming the destination directly
            // made the runtime create it, which meant a directory source produced a
            // *directory* here: progress then measured a dirent instead of data, and
            // the cancel path called `remove_file` on it and silently failed. One
            // archive for both cases removes that whole branch.
            "-".to_string(),
        ],
        Some(dest),
        None,
        TransferSubject {
            verb: "copy-out",
            target_kind: "container",
            target: container_id.clone(),
            target_name: container_path.clone(),
        },
    );
    Ok(TransferStarted {
        job_id,
        total_estimate: None,
    })
}

/// Cancel a running transfer.
///
/// Delegates to the streaming registry rather than keeping a second one: that
/// registry already owns every in-flight child, kills whole process groups, and is
/// drained when the app exits.
#[tauri::command]
pub async fn cancel_transfer(job_id: String) -> Result<registry::CancelOutcome, ColimaError> {
    Ok(cancel_transfer_job(&job_id))
}

/// Ask a transfer to stop, and say what was actually there.
///
/// Two steps because they answer different questions: the registry knows whether
/// the job exists and whether it already finished — which `cancel_stream` alone
/// reports as an indistinguishable `false` — while `cancel_stream` is what kills the
/// process. Recording intent first also covers a job that is registered but whose
/// child has not been spawned yet; `spawn_job` checks for it.
pub fn cancel_transfer_job(job_id: &str) -> registry::CancelOutcome {
    let outcome = registry::request_cancel(job_id);
    if outcome == registry::CancelOutcome::Cancelled {
        // `arm_cancel`, not `cancel_stream`: the child may not exist yet, and a
        // plain cancel would be dropped in that gap. Arming makes the kill happen
        // as soon as the child registers.
        crate::streaming_cmd::arm_cancel(job_id);
    }
    outcome
}

/// Every transfer this process knows about: unfinished, plus recent outcomes.
#[tauri::command]
pub async fn transfer_list() -> Result<Vec<registry::TransferSnapshot>, ColimaError> {
    Ok(registry::list())
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that removes itself.
    ///
    /// `Drop` rather than a call at the end of each test: a failing assertion
    /// unwinds, and cleanup that only runs on the happy path leaves litter behind
    /// exactly when someone is already debugging. `TMPDIR` is not always the
    /// system temp directory — on this project's dev setup it is the repo — so
    /// leaving anything behind is immediately in the way.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "colimaui-transfer-{}-{:?}",
                tag,
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn str(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn traversal_in_the_file_name_is_rejected() {
        let dir = TempDir::new("traversal");
        let err = resolve_destination(dir.str(), "../escaped.tar", false)
            .expect_err("must not escape the chosen folder");
        assert!(err.contains("escapes"), "unexpected error: {}", err);
    }

    #[test]
    fn nested_traversal_is_rejected_too() {
        let dir = TempDir::new("traversal-deep");
        let err = resolve_destination(dir.str(), "../../etc/passwd", false)
            .expect_err("must not escape the chosen folder");
        assert!(!err.is_empty());
    }

    #[test]
    fn a_plain_file_name_resolves_inside_the_chosen_folder() {
        let dir = TempDir::new("plain");
        let dest = resolve_destination(dir.str(), "images.tar", false).unwrap();
        assert_eq!(dest, dir.path().join("images.tar"));
    }

    #[test]
    fn existing_destination_needs_explicit_overwrite() {
        let dir = TempDir::new("overwrite");
        let existing = dir.path().join("taken.tar");
        std::fs::write(&existing, b"old").unwrap();

        let err = resolve_destination(dir.str(), "taken.tar", false)
            .expect_err("must not clobber silently");
        assert!(err.contains("already exists"), "unexpected error: {}", err);

        // With consent it resolves, and the old file is still there for the
        // command to replace.
        assert!(resolve_destination(dir.str(), "taken.tar", true).is_ok());
        assert!(existing.exists());
    }

    #[test]
    fn missing_destination_folder_is_rejected() {
        let err = resolve_destination("/no/such/folder/anywhere", "x.tar", false)
            .expect_err("must reject a folder that is not there");
        assert!(err.contains("does not exist"), "unexpected error: {}", err);
    }

    #[test]
    fn flag_like_and_blank_arguments_are_rejected() {
        // `docker save --output=/etc/x` — a leading dash changes the command.
        assert!(require_plain("Image reference", "-o/etc/x").is_err());
        assert!(require_plain("File name", "  ").is_err());
        assert!(require_plain("Container path", "/tmp/a\nb").is_err());
        assert!(require_plain("Image reference", "alpine:3.19").is_ok());
        assert!(require_plain("Container path", "/etc/hosts").is_ok());
    }

    #[test]
    fn invalid_container_id_is_refused_before_anything_runs() {
        let err = start_copy_to_container(
            None,
            "not a container id!".to_string(),
            "/etc/hosts".to_string(),
            "/tmp/x".to_string(),
        )
        .expect_err("must reject the id");
        assert!(err.contains("Invalid container id"), "unexpected: {}", err);
    }

    #[test]
    fn export_with_no_images_selected_is_refused() {
        let dir = TempDir::new("empty-selection");
        let err = start_image_save(
            None,
            vec![],
            dir.str().to_string(),
            "out.tar".to_string(),
            false,
        )
        .expect_err("must require a selection");
        assert!(err.contains("at least one"), "unexpected: {}", err);
    }

    #[test]
    fn export_validates_the_path_before_spawning_anything() {
        let dir = TempDir::new("no-spawn");
        let before = crate::streaming_cmd::active_stream_count();
        let err = start_image_save(
            None,
            vec!["alpine:3.19".to_string()],
            dir.str().to_string(),
            "../escaped.tar".to_string(),
            false,
        )
        .expect_err("must reject traversal");
        assert!(!err.is_empty());
        assert_eq!(
            crate::streaming_cmd::active_stream_count(),
            before,
            "a rejected transfer must not have started a command"
        );
    }

    #[test]
    fn loading_a_missing_archive_is_refused() {
        let err = start_image_load(None, "/no/such/archive.tar".to_string())
            .expect_err("must reject a missing archive");
        assert!(err.contains("not a file"), "unexpected: {}", err);
    }

    #[test]
    fn cancelling_an_unknown_job_reports_false() {
        assert!(!crate::streaming_cmd::cancel_stream("save-does-not-exist"));
    }

    #[test]
    fn human_bytes_is_readable() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn available_bytes_reads_the_filesystem() {
        // Cheap sanity check: the temp directory's filesystem has some room, and
        // a nonexistent path yields no answer rather than a wrong one.
        assert!(available_bytes(&std::env::temp_dir()).unwrap_or(0) > 0);
        assert!(available_bytes(Path::new("/no/such/path/at/all")).is_none());
    }

    #[test]
    fn archive_names_must_say_tar() {
        assert!(require_archive_name("images.tar").is_ok());
        assert!(require_archive_name("Images.TAR").is_ok());

        // No extension: the file the OS cannot classify and `docker load` cannot be
        // pointed at without guessing.
        let err = require_archive_name("images").expect_err("must reject a bare name");
        assert!(err.contains(".tar"), "unexpected: {}", err);
    }

    #[test]
    fn compressed_extensions_are_refused_rather_than_honoured() {
        // `docker save` writes an uncompressed archive, so honouring these names
        // would produce a file whose extension lies about its contents.
        for name in ["images.tar.gz", "images.tgz", "images.zip", "images.tar.xz"] {
            let err = require_archive_name(name).expect_err("must reject");
            assert!(
                err.contains("uncompressed"),
                "{} gave the wrong reason: {}",
                name,
                err
            );
        }
    }

    #[test]
    fn a_container_id_cannot_pose_as_a_flag() {
        // `is_valid_container_id` allows `-` because it is legal inside a name; a
        // *leading* one would change what the runtime reads the argument as.
        let err = require_container_ref("--rm").expect_err("must reject a flag-like id");
        assert!(err.contains("must not start with '-'"), "unexpected: {}", err);
        assert!(require_container_ref("my-container").is_ok());
    }

    #[test]
    fn sniff_tar_accepts_a_real_archive() {
        let dir = TempDir::new("sniff-ok");
        let path = dir.path().join("real.tar");

        // A ustar header: the magic sits at offset 257 of the first 512-byte block.
        let mut block = vec![0u8; 512];
        block[257..262].copy_from_slice(b"ustar");
        std::fs::write(&path, &block).unwrap();

        assert!(sniff_tar(&path).is_ok());
    }

    #[test]
    fn sniff_tar_names_the_compression_it_found() {
        let dir = TempDir::new("sniff-gz");

        let gz = dir.path().join("archive.tar.gz");
        std::fs::write(&gz, [0x1f, 0x8b, 0x08, 0x00]).unwrap();
        let err = sniff_tar(&gz).expect_err("gzip must not reach the runtime");
        assert!(err.contains("gzip"), "unexpected: {}", err);

        let zip = dir.path().join("archive.zip");
        std::fs::write(&zip, b"PK\x03\x04rest").unwrap();
        let err = sniff_tar(&zip).expect_err("zip must not reach the runtime");
        assert!(err.contains("ZIP"), "unexpected: {}", err);
    }

    #[test]
    fn sniff_tar_rejects_something_that_is_merely_short() {
        let dir = TempDir::new("sniff-short");
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, b"just a log line").unwrap();

        assert!(sniff_tar(&path).is_err());
    }

    #[test]
    fn a_pre_posix_tar_behind_a_tar_name_is_let_through() {
        // Old GNU tars carry no magic. Blocking them would reject archives the
        // runtime reads fine, which is worse than letting an odd file reach it.
        let dir = TempDir::new("sniff-legacy");
        let path = dir.path().join("legacy.tar");
        std::fs::write(&path, vec![b'x'; 512]).unwrap();

        assert!(sniff_tar(&path).is_ok());
    }

    #[test]
    fn copy_out_demands_an_archive_name() {
        let dir = TempDir::new("cp-out-name");
        let err = start_copy_from_container(
            None,
            "abc123".to_string(),
            "/tmp/payload.bin".to_string(),
            dir.str().to_string(),
            "payload.bin".to_string(),
            false,
        )
        .expect_err("copy out now produces an archive, so the name must say so");
        assert!(err.contains(".tar"), "unexpected: {}", err);
    }
}
