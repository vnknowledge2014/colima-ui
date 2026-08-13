//! Read Falco's output and make it legible next to the containers it is about.
//!
//! # ColimaUI is a front end for Falco, not a competitor to it
//!
//! Nothing here detects anything. Falco (CNCF, Apache-2.0) does the detecting,
//! with eBPF, a team far larger than ours, and rules maintained against real
//! attacker behaviour. What it does not have is a decent interface, or any idea
//! what a user's compose services are called. That gap is what this module
//! fills: read its events, name the container in the words the user uses, and
//! put priorities in front of them.
//!
//! The rule for every future change: if the answer to *"has Falco already done
//! this?"* is yes, we do not do it here.
//!
//! # Detection runs inside the VM, never on the host
//!
//! Two reasons, both measured 2026-08-13 (see
//! `plans/reports/from-cook-gate0-to-owner-260813-0823-detonation-and-falco-viability-report.md`):
//!
//! 1. Falco is a Linux kernel-level tool. There is no macOS host build. It runs
//!    in the Lima VM or not at all.
//! 2. `falco` on a Mac's PATH is most likely **the wrong program**:
//!    `brew info falco` resolves to a Fastly VCL parser/linter
//!    (`ysugimoto/falco`, MIT). Detecting by file existence would report that
//!    as a runtime security engine. Identity is therefore taken from the
//!    version string, not from the presence of a binary.
//!
//! # Why "is it running" is not the question
//!
//! During the gate-0 probe Falco installed, attached its eBPF driver, ran as a
//! healthy systemd unit — and detected nothing at all, because the rules fetch
//! was disabled and the engine loaded an empty rule set. A security tool that
//! is up and silently blind is worse than one that is plainly missing: the user
//! believes they are covered. So [`FalcoState`] carries a rule count, and zero
//! rules is its own state rather than a flavour of "ready".

use serde::{Deserialize, Serialize};

use crate::helpers::run_cmd;

/// Where Falco is asked to write JSON events.
///
/// Not Falco's default — its `file_output` ships disabled, so there is no
/// default to inherit. This is the path the knowledge-base article tells users
/// to configure, and the one detection reports as missing when they have not.
const DEFAULT_EVENT_FILE: &str = "/var/log/falco/events.json";

/// What detection concluded. Ordered from "nothing to work with" upward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FalcoStatus {
    /// No Falco in the VM.
    Missing,
    /// A binary called `falco` exists but is not CNCF Falco — almost certainly
    /// the Fastly VCL linter of the same name. Reported distinctly because
    /// "install Falco" is unhelpful advice to someone who already has *a*
    /// falco.
    NotFalco,
    /// Installed, but no Falco process is running.
    Installed,
    /// Running with **no rules loaded**. Detecting nothing while appearing
    /// healthy — the state the gate-0 probe fell into, and the reason this
    /// enum exists rather than a boolean.
    RunningWithoutRules,
    /// Running with rules, but not writing JSON events anywhere this app can
    /// read. Falco is protecting the user; ColimaUI just cannot show it.
    OutputNotConfigured,
    /// Running, has rules, and events are readable.
    Ready,
}

/// The answer to "can this app show the user runtime events, and if not, why".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FalcoState {
    pub status: FalcoStatus,
    /// CNCF Falco's version, when that is what is installed.
    pub version: Option<String>,
    /// How many rules the engine loads. Zero is the dangerous case.
    pub rule_count: usize,
    /// Path Falco writes JSON events to, when it is configured to.
    pub event_file: Option<String>,
    /// The Colima profile detection ran against.
    pub profile: Option<String>,
}

impl FalcoState {
    fn absent(status: FalcoStatus, profile: Option<String>) -> Self {
        Self {
            status,
            version: None,
            rule_count: 0,
            event_file: None,
            profile,
        }
    }
}

/// The Colima profile the app is currently talking to.
///
/// Falco runs in the VM, so every command below is scoped to this profile.
/// `detect_docker_host` is the same function the rest of the app uses to pick a
/// daemon, and it already excludes detonation instances — a Falco reading taken
/// from the VM running an untrusted sample would be about the wrong machine.
fn active_profile() -> Option<String> {
    crate::path_util::detect_docker_host().map(|(_, profile)| profile)
}

/// Run one command inside the VM.
fn vm_exec(profile: &str, script: &str) -> Result<String, String> {
    run_cmd("colima", &["ssh", "-p", profile, "--", "sudo", "bash", "-c", script])
}

/// Run one command inside the VM and keep its output as raw bytes.
///
/// `helpers::run_cmd` returns `String` via `from_utf8_lossy`, which is right for
/// human-readable output and wrong for anything whose **length** matters: one
/// invalid byte becomes a three-byte replacement character, and a `\r` is
/// counted here but dropped by `lines()`. Reading the event file through it made
/// the byte offset drift in both directions, silently losing or repeating
/// events on every poll. Offsets are computed from these bytes instead.
fn vm_exec_bytes(profile: &str, script: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("colima")
        .args(["ssh", "-p", profile, "--", "sudo", "bash", "-c", script])
        .output()
        .map_err(|e| format!("colima ssh: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out.stdout)
}

/// Quote a path for `bash -c`.
///
/// The event file path comes out of Falco's own config, which a user edits by
/// hand. A path with a space would otherwise split into two arguments, and one
/// with a `;` would end the command.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Current size of the event file, or `None` if it cannot be read.
pub fn event_file_size(profile: &str, path: &str) -> Option<u64> {
    vm_exec(profile, &format!("stat -c %s {} 2>/dev/null", shell_quote(path)))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Pull the CNCF version out of `falco --version`.
///
/// The whole discriminator between real Falco and the Fastly VCL linter that
/// also answers to `falco`. The linter prints its own version banner and never
/// this line, so absence is a negative identification rather than a parse miss.
fn parse_falco_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.split("Falco version:")
            .nth(1)
            .map(|rest| rest.split_whitespace().next().unwrap_or("").to_string())
            .filter(|v| !v.is_empty())
    })
}

/// Count rules from `falco -L -o json_output=true`.
///
/// Verified against Falco 0.44.1: the document has top-level `rules`, `macros`
/// and `lists` arrays, and `rules` is the one that decides whether anything can
/// be detected. Counting `macros` instead would report a healthy-looking number
/// for an engine with no rules at all.
fn parse_rule_count(json: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("rules").and_then(|r| r.as_array()).map(Vec::len))
        .unwrap_or(0)
}

/// Read the configured JSON event file path out of Falco's config.
///
/// Falco 0.38+ layers `/etc/falco/config.d/*.yaml` over `falco.yaml`, so the
/// last `filename:` under a `file_output:` block wins. A full YAML parse would
/// be more rigorous; this reads the one key it needs, and reports `None` when
/// it is not confidently found rather than inventing a path.
fn parse_event_file(config_dump: &str) -> Option<String> {
    let mut in_file_output = false;
    let mut enabled = false;
    let mut filename = None;

    for line in config_dump.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("file_output:") {
            in_file_output = true;
            continue;
        }
        // A new top-level key ends the block.
        if in_file_output && !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty()
        {
            in_file_output = false;
        }
        if !in_file_output {
            continue;
        }
        // Falco's shipped config documents every key with a trailing comment,
        // so `enabled: true  # turn this off to ...` must not read as `false`.
        if let Some(v) = trimmed.strip_prefix("enabled:") {
            enabled = strip_inline_comment(v) == "true";
        }
        if let Some(v) = trimmed.strip_prefix("filename:") {
            let v = strip_inline_comment(v).trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                filename = Some(v.to_string());
            }
        }
    }

    let path = filename.unwrap_or_else(|| DEFAULT_EVENT_FILE.to_string());
    // A relative path is resolved against Falco's working directory, which is
    // not something this app can know. Reporting it would produce a `stat` that
    // silently reads the wrong file, or nothing.
    if !path.starts_with('/') {
        return None;
    }
    enabled.then_some(path)
}

/// A YAML scalar with any trailing `# comment` removed.
fn strip_inline_comment(value: &str) -> &str {
    match value.find(" #") {
        Some(i) => value[..i].trim(),
        None => value.trim(),
    }
}

/// Ask the VM what Falco is doing.
///
/// One SSH round trip rather than four: each `colima ssh` pays connection setup,
/// and this runs behind a user opening a tab.
/// How long a detection result is reused.
///
/// Detection SSHes into the VM, dumps every rule as JSON and reads a 64 KB
/// config. The UI polls it every few seconds and the reader loop asks again
/// whenever it has no file, so uncached this was a rule dump every five seconds
/// forever. Falco's install state does not change on that timescale.
const DETECT_TTL: std::time::Duration = std::time::Duration::from_secs(30);

static DETECT_CACHE: LazyLock<std::sync::Mutex<Option<(FalcoState, std::time::Instant)>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));

/// Drop the cached detection, so the next call measures again.
///
/// The screen that reports Falco's state is also the screen telling the user how
/// to change it. Pressing Start is the moment they say they have followed those
/// instructions, so the cached "not configured" from thirty seconds ago is
/// exactly the wrong answer to keep.
pub fn invalidate_detect() {
    if let Ok(mut cache) = DETECT_CACHE.lock() {
        *cache = None;
    }
}

pub fn detect() -> FalcoState {
    if let Ok(cache) = DETECT_CACHE.lock() {
        if let Some((state, at)) = cache.as_ref() {
            if at.elapsed() < DETECT_TTL {
                return state.clone();
            }
        }
    }
    let state = detect_uncached();
    if let Ok(mut cache) = DETECT_CACHE.lock() {
        *cache = Some((state.clone(), std::time::Instant::now()));
    }
    state
}

fn detect_uncached() -> FalcoState {
    let Some(profile) = active_profile() else {
        return FalcoState::absent(FalcoStatus::Missing, None);
    };

    // Delimited so one blob can be split without a second call. `|| true` keeps
    // a missing binary from failing the whole script.
    let script = "\
echo '<<<VERSION>>>'; falco --version 2>/dev/null || true; \
echo '<<<RUNNING>>>'; (pgrep -x falco >/dev/null && echo yes || echo no); \
echo '<<<RULES>>>'; falco -L -o json_output=true 2>/dev/null || true; \
echo '<<<CONFIG>>>'; cat /etc/falco/falco.yaml /etc/falco/config.d/*.yaml 2>/dev/null || true";

    let Ok(out) = vm_exec(&profile, script) else {
        return FalcoState::absent(FalcoStatus::Missing, Some(profile));
    };

    let section = |name: &str| -> String {
        out.split(&format!("<<<{name}>>>"))
            .nth(1)
            .map(|rest| {
                rest.split("<<<")
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .unwrap_or_default()
    };

    let version_out = section("VERSION");
    if version_out.is_empty() {
        return FalcoState::absent(FalcoStatus::Missing, Some(profile));
    }
    let Some(version) = parse_falco_version(&version_out) else {
        // Something answers to `falco` but is not the CNCF one.
        return FalcoState::absent(FalcoStatus::NotFalco, Some(profile));
    };

    let running = section("RUNNING") == "yes";
    let rule_count = parse_rule_count(&section("RULES"));
    let event_file = parse_event_file(&section("CONFIG"));

    let status = if !running {
        FalcoStatus::Installed
    } else if rule_count == 0 {
        FalcoStatus::RunningWithoutRules
    } else if event_file.is_none() {
        FalcoStatus::OutputNotConfigured
    } else {
        FalcoStatus::Ready
    };

    FalcoState {
        status,
        version: Some(version),
        rule_count,
        event_file,
        profile: Some(profile),
    }
}

/// Events appended to Falco's output file since `offset`, and the new offset.
///
/// Reads by byte offset rather than tailing a long-lived process: a `tail -f`
/// held open through `colima ssh` is a child that outlives its usefulness and
/// has to be reaped, and it loses its place across a VM restart. An offset is
/// resumable and owns nothing.
///
/// A file that shrank (log rotation, or the user clearing it) restarts from
/// zero instead of reading garbage from the middle of a line.
pub fn read_since(profile: &str, path: &str, offset: u64) -> Result<(Vec<FalcoEvent>, u64), String> {
    let Some(size) = event_file_size(profile, path) else {
        return Err(format!("Cannot stat {path}"));
    };

    if size == 0 {
        return Ok((Vec::new(), 0));
    }
    // A file smaller than where we were reading was rotated or truncated;
    // anything else would read from the middle of a line.
    let start = if size < offset { 0 } else { offset };
    if size == start {
        return Ok((Vec::new(), start));
    }

    // `tail -c +N` is 1-indexed on bytes. Raw bytes, so the arithmetic below is
    // exact — see `vm_exec_bytes`.
    let chunk = vm_exec_bytes(
        profile,
        &format!("tail -c +{} {}", start + 1, shell_quote(path)),
    )?;

    let (events, consumed) = consume_complete_lines(&chunk);
    Ok((events, start + consumed as u64))
}

/// Parse the complete lines in a chunk, and report how many **bytes** they used.
///
/// Split out so the offset arithmetic is testable against real byte sequences
/// rather than against a restatement of it. The previous test for this rebuilt
/// the logic in a closure and asserted on the copy, which would have passed no
/// matter what this function did.
///
/// Only whole lines are consumed: Falco appends line by line, but a read can
/// still land mid-write, and half a JSON object parsed now is lost rather than
/// retried. Byte positions throughout — a char count drifts on multi-byte input
/// and on `\r`.
fn consume_complete_lines(chunk: &[u8]) -> (Vec<FalcoEvent>, usize) {
    let Some(last_newline) = chunk.iter().rposition(|b| *b == b'\n') else {
        // Not one complete line yet. Stay exactly where we were.
        return (Vec::new(), 0);
    };
    let consumed = last_newline + 1;
    let events = chunk[..consumed]
        .split(|b| *b == b'\n')
        .filter_map(|line| parse_event(&String::from_utf8_lossy(line)))
        .collect();
    (events, consumed)
}

/// Falco's own priority ladder, most severe first.
///
/// Kept as Falco names them rather than remapped onto the app's own severity
/// vocabulary: these strings appear in the user's rule files, and a user
/// comparing this screen to their own Falco config must see the same words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalcoPriority {
    Debug,
    Informational,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl FalcoPriority {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "emergency" => Some(Self::Emergency),
            "alert" => Some(Self::Alert),
            "critical" => Some(Self::Critical),
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "notice" => Some(Self::Notice),
            // Falco writes "Informational"; older rule packs write "Info".
            "informational" | "info" => Some(Self::Informational),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Emergency => "emergency",
            Self::Alert => "alert",
            Self::Critical => "critical",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Notice => "notice",
            Self::Informational => "informational",
            Self::Debug => "debug",
        }
    }
}

/// One Falco alert.
///
/// The shape is Falco's, deliberately. Inventing our own detection schema would
/// mean mapping their fields onto ours, and every mapping is a chance to lose
/// the field that mattered. `fields` keeps the raw `output_fields` so the detail
/// view can show what the rule actually matched on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FalcoEvent {
    /// Falco's RFC3339 timestamp, kept verbatim.
    pub time: String,
    /// Milliseconds since epoch, derived for sorting and retention.
    pub ts_ms: i64,
    pub priority: FalcoPriority,
    pub rule: String,
    /// The human-readable line Falco composed from the rule's output template.
    pub output: String,
    pub source: String,
    pub tags: Vec<String>,
    pub container_id: Option<String>,
    pub container_name: Option<String>,
    pub image: Option<String>,
    pub fields: serde_json::Value,
}

/// Falco writes `container.id` as the literal `host` for events that did not
/// happen inside a container. Treating that as an id would make every host
/// event look like a container called "host".
const HOST_SENTINEL: &str = "host";

fn field_str(fields: &serde_json::Value, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty() && s != "<NA>")
}

/// Parse one line of Falco JSON output.
///
/// Tolerant by design. Falco's output is shaped by rules the *user* writes, so a
/// missing field, an unknown rule or an unknown priority are all normal states
/// rather than corruption — and one odd line must never stop the stream. Only a
/// line that is not JSON, or carries no rule at all, is refused.
pub fn parse_event(line: &str) -> Option<FalcoEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let raw: serde_json::Value = serde_json::from_str(line).ok()?;

    let rule = raw.get("rule")?.as_str()?.to_string();
    if rule.is_empty() {
        return None;
    }

    let fields = raw
        .get("output_fields")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let time = raw
        .get("time")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let container_id = field_str(&fields, "container.id").filter(|id| id != HOST_SENTINEL);
    let container_name = field_str(&fields, "container.name").filter(|n| n != HOST_SENTINEL);
    let image = match (
        field_str(&fields, "container.image.repository"),
        field_str(&fields, "container.image.tag"),
    ) {
        (Some(repo), Some(tag)) => Some(format!("{repo}:{tag}")),
        (Some(repo), None) => Some(repo),
        _ => None,
    };

    Some(FalcoEvent {
        ts_ms: parse_time_ms(&time, &fields),
        time,
        // An unrecognised priority is reported as the mildest rather than
        // dropped: a rule using a custom word must still reach the user, and
        // guessing "critical" would cry wolf.
        priority: raw
            .get("priority")
            .and_then(|v| v.as_str())
            .and_then(FalcoPriority::parse)
            .unwrap_or(FalcoPriority::Debug),
        rule,
        output: raw
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        source: raw
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("syscall")
            .to_string(),
        tags: raw
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        container_id,
        container_name,
        image,
        fields,
    })
}

/// Milliseconds since epoch for an event.
///
/// Prefers `evt.time`, which Falco emits as integer **nanoseconds** — dividing
/// rather than parsing text avoids a date parser in the hot path. Falls back to
/// the RFC3339 string, and finally to zero so an event with an unreadable clock
/// is still shown rather than dropped.
fn parse_time_ms(time: &str, fields: &serde_json::Value) -> i64 {
    if let Some(nanos) = fields.get("evt.time").and_then(|v| v.as_i64()) {
        return nanos / 1_000_000;
    }
    chrono::DateTime::parse_from_rfc3339(time)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

/// A container named the way the user names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRef {
    pub container_id: String,
    pub container_name: Option<String>,
    /// Compose project, when the container belongs to one.
    pub project: Option<String>,
    /// Compose service, when the container belongs to one.
    pub service: Option<String>,
}

impl ServiceRef {
    /// What to show in a list: the most specific name available.
    ///
    /// A raw container id is the last resort, not the default. It is also the
    /// correct answer for a container that has since exited — that event still
    /// happened, and hiding it because the container is gone would drop exactly
    /// the events most worth reading.
    pub fn label(&self) -> String {
        match (&self.project, &self.service) {
            (Some(p), Some(s)) => format!("{p}/{s}"),
            _ => self
                .container_name
                .clone()
                .unwrap_or_else(|| self.container_id.clone()),
        }
    }
}

fn label_of(container: &serde_json::Value, key: &str) -> Option<String> {
    let labels = container.get("Labels")?;
    // `docker ps --format json` gives labels either as a map or as one
    // comma-joined string, depending on version. Both appear in this repo's own
    // fixtures, so both are read here.
    if let Some(map) = labels.as_object() {
        return map
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty());
    }
    labels.as_str()?.split(',').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k.trim() == key && !v.is_empty()).then(|| v.trim().to_string())
    })
}

/// Match an event to a running container.
///
/// Falco reports a short container id; `docker ps` reports a short id too, but
/// an id from another source may be the full 64-character one, so the comparison
/// is by prefix in whichever direction is longer.
pub fn correlate(event: &FalcoEvent, containers: &[serde_json::Value]) -> Option<ServiceRef> {
    let id = event.container_id.as_deref()?;

    // A container id shorter than this is not something to match a prefix
    // against. An empty `ID` made `id.starts_with(cid)` true for every event,
    // attaching whichever container happened to be listed first to all of them.
    const MIN_ID_LEN: usize = 8;

    let found = containers.iter().find(|c| {
        c.get("ID")
            .or_else(|| c.get("Id"))
            .and_then(|v| v.as_str())
            .filter(|cid| cid.len() >= MIN_ID_LEN)
            .is_some_and(|cid| cid.starts_with(id) || id.starts_with(cid))
    });

    let Some(container) = found else {
        // Known id, unknown container: it exited, or it belongs to another
        // daemon. Still worth showing, with the only name we have.
        return Some(ServiceRef {
            container_id: id.to_string(),
            container_name: event.container_name.clone(),
            project: None,
            service: None,
        });
    };

    Some(ServiceRef {
        container_id: id.to_string(),
        container_name: container
            .get("Names")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| event.container_name.clone()),
        project: label_of(container, "com.docker.compose.project"),
        service: label_of(container, "com.docker.compose.service"),
    })
}

// ---------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

static WATCHING: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
static SHOULD_WATCH: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

/// How often the event file is checked.
const POLL_SECS: u64 = 5;

/// Priority at or above which an event becomes an alert.
///
/// Critical rather than Warning: the default rule pack emits Warning for things
/// as ordinary as reading `/etc/shadow` during a package install, and an alert
/// stream that cries wolf is one the user turns off.
const ALERT_AT: FalcoPriority = FalcoPriority::Critical;

/// Whether the reader is running.
pub fn is_watching() -> bool {
    WATCHING.load(Ordering::Relaxed)
}

/// Turn the reader on or off.
///
/// Turning it **off** is never gated: a background loop a lapsed user can see
/// and cannot stop is the shape of malware, and `security_watch` sets the same
/// precedent for its own switch.
pub fn set_watching(enabled: bool) -> Result<bool, String> {
    if enabled && !crate::commands::metrics_store::entitled_now() {
        return Err("Runtime events require an active subscription".to_string());
    }
    // The user has just acted on whatever the screen told them to change, so a
    // cached reading from before that is stale by definition.
    invalidate_detect();
    SHOULD_WATCH.store(enabled, Ordering::Relaxed);
    if enabled && !WATCHING.swap(true, Ordering::Relaxed) {
        spawn_reader();
    }
    Ok(enabled)
}

/// Whether a newly detected event file should be read from its end.
///
/// The rule, and why it is a named function rather than an inline condition:
/// this decision was wrong twice. Reading a fresh file from byte zero replays
/// weeks of history as if it had just happened; skipping to the end of a file we
/// were already reading throws away everything written during the outage that
/// caused the re-detect. Both failures are silent, so the logic is tested rather
/// than trusted.
///
/// Seek to the end when this is the first sight of a file, or when the path
/// changed. Keep the offset only when re-attaching to the same path.
fn should_seek_to_end(previous: Option<&str>, current: Option<&str>) -> bool {
    match (previous, current) {
        // Re-attaching to the file we were already reading: keep our place.
        (Some(prev), Some(cur)) if prev == cur => false,
        _ => true,
    }
}

/// Poll Falco's output file, correlate, store, and alert.
///
/// Entitlement is re-read every pass rather than once at startup: a
/// subscription that lapses while the app is open must stop the work, not
/// merely hide the page.
fn spawn_reader() {
    tauri::async_runtime::spawn(async move {
        let mut offset: u64 = 0;
        let mut file: Option<String> = None;
        let mut profile: Option<String> = None;
        // Consecutive read failures. A single hiccup must not trigger a
        // re-detect, because a re-detect can move where we resume from.
        let mut failures: u32 = 0;
        // First pass must detect; later passes only when reading has broken.
        let mut needs_redetect = true;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;

            if !SHOULD_WATCH.load(Ordering::Relaxed) {
                break;
            }
            if !crate::commands::metrics_store::entitled_now_cached() {
                eprintln!("[falco] entitlement lapsed; stopping the event reader");
                break;
            }

            // Resolve the file once, then re-resolve if reading stops working:
            // the user may enable file output while this is running.
            //
            // The trigger is a flag rather than `file.is_none()`. Clearing
            // `file` to request a re-detect destroyed the one fact the resume
            // logic below needs — which file we were reading — so the
            // "same file, keep the offset" branch could never be taken and every
            // reconnect silently skipped to the end.
            if needs_redetect {
                let state = tokio::task::spawn_blocking(detect)
                    .await
                    .unwrap_or_else(|_| FalcoState::absent(FalcoStatus::Missing, None));
                let previous_file = file.clone();
                file = state.event_file.clone();
                profile = state.profile.clone();
                needs_redetect = file.is_none();

                // Where to resume depends on whether this is the first sight of
                // this file or a reconnection to one we were already reading.
                //
                // First attach starts at the END. The event file is a log that
                // may hold weeks of history; reading from zero stored all of it
                // and fired an alert for each, as if everything that ever
                // happened had just happened.
                //
                // Re-attaching to the *same* path keeps the offset. Seeking to
                // the end again would silently discard everything Falco wrote
                // during the outage — on a security screen, losing events
                // without saying so is worse than the outage was.
                if should_seek_to_end(previous_file.as_deref(), file.as_deref()) {
                    offset = match (&file, &profile) {
                        (Some(p), Some(prof)) => {
                            let (p, prof) = (p.clone(), prof.clone());
                            tokio::task::spawn_blocking(move || event_file_size(&prof, &p))
                                .await
                                .ok()
                                .flatten()
                                .unwrap_or(0)
                        }
                        _ => 0,
                    };
                } else {
                    eprintln!("[falco] reconnected to {file:?}; resuming at byte {offset}");
                }
                failures = 0;
                continue;
            }

            let (Some(path), Some(prof)) = (file.clone(), profile.clone()) else {
                continue;
            };

            let read = tokio::task::spawn_blocking(move || read_since(&prof, &path, offset)).await;
            match read {
                Ok(Ok((events, next_offset))) => {
                    failures = 0;
                    offset = next_offset;
                    if !events.is_empty() {
                        ingest(events).await;
                    }
                }
                _ => {
                    // Tolerate a few failures before re-detecting. A busy VM or
                    // a momentary ssh error is not a reason to forget where we
                    // were reading.
                    failures += 1;
                    if failures >= 3 {
                        eprintln!("[falco] event file unreadable {failures}x; re-detecting");
                        // `file` is deliberately left set: it is what the resume
                        // logic compares against to decide whether it may keep
                        // the offset.
                        needs_redetect = true;
                        failures = 0;
                    }
                }
            }
        }

        WATCHING.store(false, Ordering::Relaxed);
    });
}

/// Correlate a batch, store it, publish it, and alert on the severe ones.
async fn ingest(events: Vec<FalcoEvent>) {
    let containers = crate::commands::containers::list_containers_cli(false)
        .await
        .unwrap_or_default();

    let labelled: Vec<(FalcoEvent, Option<String>)> = events
        .into_iter()
        .map(|e| {
            let label = correlate(&e, &containers).map(|r| r.label());
            (e, label)
        })
        .collect();

    let now = chrono::Utc::now().timestamp_millis();
    if let Err(e) = tokio::task::spawn_blocking({
        let batch = labelled.clone();
        move || crate::commands::security_history::record_falco_events(&batch, now)
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
    {
        eprintln!("[falco] could not store events: {e}");
    }

    for (event, label) in &labelled {
        crate::sse::publish_sse_event(
            "falco.event",
            &serde_json::json!({ "event": event, "label": label }),
        );
    }

    // Alerts go through the existing path rather than a second pipeline: one
    // place alerts arrive, one log, one set of notification settings.
    //
    // Grouped, because `emit_security` requires it: a rule matching in a loop
    // can fire hundreds of times a minute, and hundreds of notifications is the
    // same as none. One alert per (rule, subject) per cooldown, with the
    // suppressed count carried in the alert itself so the number is visible
    // rather than quietly discarded.
    let mut fired: Vec<(String, String, FalcoPriority, i64, usize)> = Vec::new();
    for (event, label) in &labelled {
        if event.priority < ALERT_AT {
            continue;
        }
        let subject = label.clone().unwrap_or_else(|| "host".to_string());
        match fired
            .iter_mut()
            .find(|(rule, subj, ..)| *rule == event.rule && *subj == subject)
        {
            Some((_, _, priority, ts, count)) => {
                *count += 1;
                if event.priority > *priority {
                    *priority = event.priority;
                }
                *ts = (*ts).max(event.ts_ms);
            }
            None => fired.push((event.rule.clone(), subject, event.priority, event.ts_ms, 1)),
        }
    }

    // The cooldown is measured on the host clock, not on the event's. `ts_ms`
    // comes from the VM, and a VM's clock jumps — Lima resyncs it after the Mac
    // sleeps. One event carrying a future timestamp would write a future `last`
    // and silence that rule until wall time caught up, which on a security
    // screen is a suppression nobody asked for and nobody would see.
    let host_now = chrono::Utc::now().timestamp_millis();
    for (rule, subject, priority, ts, count) in fired {
        if !should_alert(&rule, &subject, host_now) {
            continue;
        }
        let name = if count > 1 {
            format!("Falco: {rule} (x{count})")
        } else {
            format!("Falco: {rule}")
        };
        crate::commands::alerts::emit_security(
            0,
            &name,
            &subject,
            crate::commands::alerts::AlertMetric::RuntimeEvent,
            priority as i64 as f64,
            ALERT_AT as i64 as f64,
            ts,
        );
    }
}

/// Silence after a (rule, subject) alerts, so one ongoing incident is one
/// notification rather than one per poll.
const ALERT_COOLDOWN_MS: i64 = 5 * 60 * 1000;

static ALERT_COOLDOWN: LazyLock<std::sync::Mutex<std::collections::HashMap<String, i64>>> =
    LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Whether this (rule, subject) is outside its cooldown.
///
/// `now_ms` must be the **host** clock. Passing the event's own timestamp lets a
/// VM clock jump silence a rule for as long as the skew lasts.
fn should_alert(rule: &str, subject: &str, now_ms: i64) -> bool {
    let ts_ms = now_ms;
    let key = format!("{rule}\u{0}{subject}");
    let Ok(mut seen) = ALERT_COOLDOWN.lock() else {
        return true;
    };
    if let Some(last) = seen.get(&key) {
        if ts_ms.saturating_sub(*last) < ALERT_COOLDOWN_MS {
            return false;
        }
    }
    // Bounded: a pathological rule set must not grow this without limit.
    if seen.len() > 500 {
        let cutoff = ts_ms - ALERT_COOLDOWN_MS;
        seen.retain(|_, last| *last >= cutoff);
    }
    seen.insert(key, ts_ms);
    true
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn falco_state() -> Result<FalcoState, crate::error::ColimaError> {
    tokio::task::spawn_blocking(detect)
        .await
        .map_err(|e| crate::error::ColimaError::from(e.to_string()))
}

#[tauri::command]
pub async fn falco_events(
    limit: Option<i64>,
) -> Result<Vec<crate::commands::security_history::StoredFalcoEvent>, crate::error::ColimaError> {
    crate::commands::security_history::falco_events(limit.unwrap_or(200))
        .map_err(crate::error::ColimaError::from)
}

#[tauri::command]
pub async fn falco_set_watching(enabled: bool) -> Result<bool, crate::error::ColimaError> {
    set_watching(enabled).map_err(crate::error::ColimaError::from)
}

#[tauri::command]
pub async fn falco_watching() -> Result<bool, crate::error::ColimaError> {
    Ok(is_watching())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real event, captured from Falco 0.44.1 running under Colima on macOS
    /// ARM during the phase 7 gate-0 probe. Kept verbatim: a hand-written
    /// fixture would only prove the parser matches our idea of the format.
    const REAL_CONTAINER_EVENT: &str = r#"{"hostname":"colima-detonate-probe","output":"08:29:44.319538044: Warning Sensitive file opened for reading by non-trusted program | file=/etc/shadow gparent=containerd-shim evt_type=openat user=root user_uid=0 process=cat proc_exepath=/bin/busybox parent=sh command=cat /etc/shadow terminal=0 container_id=01cdef08d09b container_name=reverent_mahavira container_image_repository=alpine container_image_tag=latest","output_fields":{"container.id":"01cdef08d09b","container.image.repository":"alpine","container.image.tag":"latest","container.name":"reverent_mahavira","evt.time":1786584584319538044,"evt.type":"openat","fd.name":"/etc/shadow","k8s.ns.name":null,"k8s.pod.name":null,"proc.cmdline":"cat /etc/shadow","proc.exepath":"/bin/busybox","proc.name":"cat","proc.pname":"sh","proc.tty":0,"user.loginuid":-1,"user.name":"root","user.uid":0},"priority":"Warning","rule":"Read sensitive file untrusted","source":"syscall","tags":["T1555","container","filesystem","host","maturity_stable","mitre_credential_access"],"time":"2026-08-13T01:29:44.319538044Z"}"#;

    /// The same rule firing on the host rather than in a container.
    const REAL_HOST_EVENT: &str = r#"{"hostname":"colima-detonate-probe","output":"08:29:44.187036508: Warning Sensitive file opened for reading by non-trusted program | file=/etc/shadow","output_fields":{"container.id":"host","container.image.repository":"","container.image.tag":"","container.name":"host","evt.time":1786584584187036508,"evt.type":"openat","fd.name":"/etc/shadow","proc.cmdline":"cat /etc/shadow","proc.name":"cat","user.name":"root","user.uid":0},"priority":"Warning","rule":"Read sensitive file untrusted","source":"syscall","tags":["T1555","container","filesystem"],"time":"2026-08-13T01:29:44.187036508Z"}"#;

    #[test]
    fn a_real_container_event_parses_with_the_fields_correlation_needs() {
        let e = parse_event(REAL_CONTAINER_EVENT).expect("real event must parse");
        assert_eq!(e.rule, "Read sensitive file untrusted");
        assert_eq!(e.priority, FalcoPriority::Warning);
        assert_eq!(e.container_id.as_deref(), Some("01cdef08d09b"));
        assert_eq!(e.container_name.as_deref(), Some("reverent_mahavira"));
        assert_eq!(e.image.as_deref(), Some("alpine:latest"));
        assert_eq!(e.source, "syscall");
        assert!(e.tags.contains(&"mitre_credential_access".to_string()));
        // evt.time is nanoseconds; the ms figure must be sane, not 1000x off.
        assert_eq!(e.ts_ms, 1_786_584_584_319);
    }

    #[test]
    fn a_host_event_is_not_reported_as_a_container_called_host() {
        let e = parse_event(REAL_HOST_EVENT).expect("host event must parse");
        assert_eq!(e.container_id, None, "'host' is a sentinel, not an id");
        assert_eq!(e.container_name, None);
        assert_eq!(e.image, None, "empty repository must not become an image name");
    }

    #[test]
    fn a_malformed_or_partial_line_never_stops_the_stream() {
        assert!(parse_event("").is_none());
        assert!(parse_event("not json at all").is_none());
        assert!(parse_event("{}").is_none(), "no rule means nothing to show");
        assert!(parse_event(r#"{"rule":""}"#).is_none());

        // Missing everything except the rule: still shown, not dropped.
        let sparse = parse_event(r#"{"rule":"Custom rule"}"#).expect("sparse event survives");
        assert_eq!(sparse.rule, "Custom rule");
        assert_eq!(sparse.priority, FalcoPriority::Debug);
        assert_eq!(sparse.source, "syscall");
        assert!(sparse.output.is_empty());

        // A priority Falco never emits must not be guessed upward.
        let odd = parse_event(r#"{"rule":"R","priority":"Spicy"}"#).expect("odd priority");
        assert_eq!(odd.priority, FalcoPriority::Debug);

        // A multi-line output field is legal JSON and must survive intact.
        let multi = parse_event(r#"{"rule":"R","output":"line one\nline two"}"#).expect("multiline");
        assert_eq!(multi.output, "line one\nline two");
    }

    #[test]
    fn priorities_order_so_a_minimum_threshold_means_something() {
        assert!(FalcoPriority::Critical > FalcoPriority::Warning);
        assert!(FalcoPriority::Emergency > FalcoPriority::Critical);
        assert!(FalcoPriority::Debug < FalcoPriority::Informational);
        // "Info" is the older spelling of the same level.
        assert_eq!(FalcoPriority::parse("Info"), Some(FalcoPriority::Informational));
        assert_eq!(
            FalcoPriority::parse("INFORMATIONAL"),
            Some(FalcoPriority::Informational)
        );
        assert_eq!(FalcoPriority::parse("nonsense"), None);
    }

    #[test]
    fn correlation_turns_a_container_id_into_a_compose_service() {
        let event = parse_event(REAL_CONTAINER_EVENT).expect("event");
        let containers = vec![serde_json::json!({
            "ID": "01cdef08d09b",
            "Names": "web-api-1",
            "Labels": {
                "com.docker.compose.project": "web",
                "com.docker.compose.service": "api"
            }
        })];

        let r = correlate(&event, &containers).expect("must correlate");
        assert_eq!(r.project.as_deref(), Some("web"));
        assert_eq!(r.service.as_deref(), Some("api"));
        assert_eq!(r.label(), "web/api", "the user's words, not a container id");
    }

    #[test]
    fn labels_are_read_whether_docker_gives_a_map_or_a_joined_string() {
        let event = parse_event(REAL_CONTAINER_EVENT).expect("event");
        let containers = vec![serde_json::json!({
            "ID": "01cdef08d09b",
            "Names": "web-api-1",
            "Labels": "a=1,com.docker.compose.project=web,com.docker.compose.service=api,b=2"
        })];
        let r = correlate(&event, &containers).expect("must correlate");
        assert_eq!(r.label(), "web/api");
    }

    #[test]
    fn a_dead_container_still_reports_its_event() {
        // The events most worth reading are often from containers that are gone.
        let event = parse_event(REAL_CONTAINER_EVENT).expect("event");
        let r = correlate(&event, &[]).expect("an unknown container still correlates");
        assert_eq!(r.project, None);
        assert_eq!(r.container_id, "01cdef08d09b");
        assert_eq!(r.label(), "reverent_mahavira");

        // With no name either, the raw id is the honest answer.
        let bare = FalcoEvent {
            container_name: None,
            ..event
        };
        assert_eq!(correlate(&bare, &[]).expect("still correlates").label(), "01cdef08d09b");
    }

    #[test]
    fn a_full_length_container_id_matches_falcos_short_one() {
        let event = parse_event(REAL_CONTAINER_EVENT).expect("event");
        let containers = vec![serde_json::json!({
            "ID": "01cdef08d09b4f2a8c1e5d7b3a9e0000000000000000000000000000000000",
            "Names": "solo",
            "Labels": {}
        })];
        assert!(correlate(&event, &containers).is_some());
    }

    /// Verbatim `falco --version` output from the gate-0 probe (Falco 0.44.1,
    /// aarch64, inside Colima).
    const REAL_VERSION_OUTPUT: &str = "\
Thu Aug 13 09:11:11 2026: Falco version: 0.44.1 (aarch64)
Thu Aug 13 09:11:11 2026: Falco initialized with configuration files:
Thu Aug 13 09:11:11 2026:    /etc/falco/config.d/engine-kind-falcoctl.yaml | schema validation: ok
Falco version: 0.44.1
Libs version:  0.25.4";

    #[test]
    fn the_cncf_version_line_is_what_identifies_falco() {
        assert_eq!(parse_falco_version(REAL_VERSION_OUTPUT).as_deref(), Some("0.44.1"));

        // The Fastly VCL linter that also answers to `falco`. It must NOT be
        // mistaken for a runtime security engine — see the module note.
        let fastly = "falco version 2.3.0\nVCL parser and linter for Fastly";
        assert_eq!(parse_falco_version(fastly), None);

        assert_eq!(parse_falco_version(""), None);
        assert_eq!(parse_falco_version("command not found"), None);
    }

    #[test]
    fn rules_are_counted_from_the_rules_array_and_nothing_else() {
        // Shape verified against `falco -L -o json_output=true` on 0.44.1: the
        // document also carries `macros` and `lists`, which are far more
        // numerous and would flatter an engine that has no rules.
        let doc = r#"{"lists":[{"a":1},{"b":2},{"c":3}],"macros":[{"m":1},{"m":2}],
                      "rules":[{"r":1},{"r":2},{"r":3},{"r":4}],
                      "required_engine_version":"0.62.0"}"#;
        assert_eq!(parse_rule_count(doc), 4);

        // The state that matters: rules present in name only.
        assert_eq!(parse_rule_count(r#"{"lists":[{"a":1}],"macros":[{"m":1}],"rules":[]}"#), 0);
        // Falco erroring out is zero rules, not a crash.
        assert_eq!(parse_rule_count(""), 0);
        assert_eq!(parse_rule_count("Error: Option does not exist"), 0);
    }

    #[test]
    fn the_event_file_is_only_reported_when_output_is_actually_enabled() {
        // Falco ships `file_output.enabled: false`, so the default install has
        // nowhere for this app to read from. Reporting a path anyway would make
        // "no events" look like "nothing happened".
        let shipped = "\
json_output: false
file_output:
  # -- Enable sending alerts to a file.
  enabled: false
  keep_alive: false
  filename: ./events.txt
program_output:
  enabled: false";
        assert_eq!(parse_event_file(shipped), None);

        let configured = "\
json_output: true
file_output:
  enabled: true
  keep_alive: false
  filename: /var/log/falco/events.json
stdout_output:
  enabled: true";
        assert_eq!(
            parse_event_file(configured).as_deref(),
            Some("/var/log/falco/events.json")
        );

        // Enabled but no filename: fall back to the path the KB article uses
        // rather than reporting "configured" with nowhere to read.
        let no_name = "file_output:\n  enabled: true\n";
        assert_eq!(parse_event_file(no_name).as_deref(), Some(DEFAULT_EVENT_FILE));

        // `enabled: true` belonging to a *different* block must not be read as
        // file output being on.
        let other_block = "\
file_output:
  enabled: false
stdout_output:
  enabled: true
  filename: /nope";
        assert_eq!(parse_event_file(other_block), None);

        // Falco's shipped config documents every key with a trailing comment.
        // Reading `true  # ...` as anything but true disabled the whole feature
        // for anyone who edited the file in place.
        let commented = "\
file_output:
  enabled: true  # -- Enable sending alerts to a file.
  filename: /var/log/falco/events.json  # -- Path to the file.";
        assert_eq!(
            parse_event_file(commented).as_deref(),
            Some("/var/log/falco/events.json")
        );

        // A relative path resolves against Falco's working directory, which we
        // cannot know — reporting it would stat the wrong file.
        let relative = "file_output:\n  enabled: true\n  filename: ./events.txt\n";
        assert_eq!(parse_event_file(relative), None);
    }

    #[test]
    fn a_blank_container_id_does_not_match_every_event() {
        // `"".starts_with(anything)` is false, but `id.starts_with("")` is
        // TRUE — so a container listed with a blank ID used to be attached to
        // every event that arrived.
        let event = parse_event(REAL_CONTAINER_EVENT).expect("event");
        let containers = vec![
            serde_json::json!({ "ID": "", "Names": "wrong-one", "Labels": {} }),
            serde_json::json!({ "ID": "01cdef08d09b", "Names": "right-one", "Labels": {} }),
        ];
        let r = correlate(&event, &containers).expect("correlates");
        assert_eq!(r.container_name.as_deref(), Some("right-one"));
    }

    #[test]
    fn only_complete_lines_are_consumed_and_the_offset_counts_bytes() {
        // Calls the production function. The previous version of this test
        // rebuilt the logic in a local closure and asserted on the copy, so it
        // would have passed whatever `consume_complete_lines` actually did.
        let ev = |rule: &str| format!(r#"{{"rule":"{rule}","priority":"Warning"}}"#);

        // Two complete events plus a partial write.
        let chunk = format!("{}\n{}\npartial{{\"rule\"", ev("one"), ev("two"));
        let (events, consumed) = consume_complete_lines(chunk.as_bytes());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].rule, "one");
        assert_eq!(
            consumed,
            chunk.rfind('\n').expect("newline") + 1,
            "the partial line must be left for the next read"
        );

        // Multi-byte content: byte length exceeds char length, and the offset
        // must be in bytes or it drifts backwards forever.
        let multi = format!("{}\n", ev("café — ☕"));
        let (events, consumed) = consume_complete_lines(multi.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(consumed, multi.len(), "bytes, not chars");
        assert!(multi.len() > multi.chars().count(), "fixture must be multi-byte");

        // Nothing complete yet: stay exactly where we were.
        let (events, consumed) = consume_complete_lines(b"{\"rule\":\"incomp");
        assert!(events.is_empty());
        assert_eq!(consumed, 0);

        // CRLF: `\r` is a byte that must be counted even though `lines()` drops it.
        let crlf = format!("{}\r\n", ev("crlf"));
        let (_, consumed) = consume_complete_lines(crlf.as_bytes());
        assert_eq!(consumed, crlf.len());

        // Blank lines are bytes too.
        let blanks = format!("\n\n{}\n", ev("after blanks"));
        let (events, consumed) = consume_complete_lines(blanks.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(consumed, blanks.len());
    }

    #[test]
    fn reconnecting_to_the_same_file_keeps_our_place() {
        let f = Some("/var/log/falco/events.json");

        // The bug this exists to prevent: a transient read failure triggers a
        // re-detect, and skipping to the end again discards everything Falco
        // wrote during the outage — silently, on a security screen.
        assert!(
            !should_seek_to_end(f, f),
            "re-attaching to the same file must keep the offset"
        );

        // First sight of a file: start at the end, or weeks of history replay
        // as if it had just happened.
        assert!(should_seek_to_end(None, f));

        // A different file has no offset of ours to keep.
        assert!(should_seek_to_end(Some("/var/log/falco/old.json"), f));

        // Nothing configured, or output turned off.
        assert!(should_seek_to_end(f, None));
        assert!(should_seek_to_end(None, None));
    }

    #[test]
    fn an_ongoing_incident_alerts_once_per_cooldown() {
        let t0 = 1_800_000_000_000_i64;
        assert!(should_alert("Terminal shell", "web/api", t0), "first fires");
        assert!(
            !should_alert("Terminal shell", "web/api", t0 + 1_000),
            "a second later is the same incident"
        );
        assert!(
            !should_alert("Terminal shell", "web/api", t0 + ALERT_COOLDOWN_MS - 1),
            "still inside the cooldown"
        );
        assert!(
            should_alert("Terminal shell", "web/api", t0 + ALERT_COOLDOWN_MS + 1),
            "past the cooldown it is news again"
        );
        // A different subject, and a different rule, are different incidents.
        assert!(should_alert("Terminal shell", "web/db", t0));
        assert!(should_alert("Write below etc", "web/api", t0));
    }

    #[test]
    fn a_host_event_correlates_to_nothing_rather_than_to_a_random_container() {
        let event = parse_event(REAL_HOST_EVENT).expect("event");
        let containers = vec![serde_json::json!({ "ID": "01cdef08d09b", "Names": "web" })];
        assert!(correlate(&event, &containers).is_none());
    }
}
