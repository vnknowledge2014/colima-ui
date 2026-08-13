//! Vulnerability scanning for local images.
//!
//! Answers one question: *what known vulnerabilities are in this image?* No
//! scoring (phase 2), no UI (phase 3), no AI (phase 4).
//!
//! ## Why Trivy, and only Trivy
//!
//! Measured against Grype on 23 real images before any of this was written
//! (`plans/reports/from-security-phase1-to-owner-260812-1259-scanner-bakeoff-report.md`).
//! The decisive finding was not speed: **Grype does not read the Docker context,
//! only `DOCKER_HOST`**, which Colima leaves empty — so it silently falls back to
//! pulling from a public registry and sends the user's private image names to
//! Docker Hub. Trivy reads the local daemon. Trivy also emits SBOMs natively,
//! which removed a second binary (`syft`) from this phase entirely.
//!
//! So `ScannerKind` has one variant. An abstraction for a second tool would be
//! scaffolding for a decision nobody has made.
//!
//! ## Scan failure is a normal result
//!
//! Trivy failed on 2 of 23 images in the bakeoff (`file blobs/sha256/… not found
//! in tar`). One image the scanner cannot read must produce one readable error,
//! never a broken list — so scanning is per-image and the error text survives to
//! the caller.
//!
//! ## What never happens here
//!
//! No automatic scanning: a scan spawns a process and reads an entire image, so
//! it is something the user asks for. Nothing is pulled — an image that is not
//! present is an error saying so, because pulling is bandwidth the user did not
//! agree to spend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::error::ColimaError;
use crate::streaming_cmd::{self, OutputSink};

/// The scanner this build drives. One variant on purpose — see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScannerKind {
    Trivy,
}

/// Normalised severity.
///
/// A closed enum rather than a string: scanners disagree on spelling (Trivy
/// writes `CRITICAL`, Grype writes `Critical` and adds `Negligible`), and a
/// string flowing through would make phase 2's scoring guess and phase 5's
/// thresholds fail quietly. Normalisation happens at the parse boundary, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Unknown,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Anything unrecognised becomes `Unknown` rather than being dropped. A
    /// vocabulary this build has not seen is still a finding.
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "HIGH" | "IMPORTANT" => Severity::High,
            "MEDIUM" | "MODERATE" => Severity::Medium,
            "LOW" | "NEGLIGIBLE" => Severity::Low,
            _ => Severity::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// CVE or advisory id.
    pub id: String,
    pub package: String,
    pub installed_version: String,
    /// Absent when no fix has been published — which is what makes a finding
    /// actionable or not, so it is kept distinct from an empty string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_version: Option<String>,
    pub severity: Severity,
    /// Which advisory database said so (`alpine`, `ghsa`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// What the user typed. Kept for display; never the cache key.
    pub image_ref: String,
    /// Content identity. The cache key, because a tag moves and a digest does
    /// not: scan `myapp:latest`, rebuild, scan again — same tag, different image.
    pub image_digest: String,
    pub findings: Vec<Finding>,
    pub scanner: ScannerKind,
    pub scanner_version: String,
    /// When the scanner's vulnerability database was last updated.
    ///
    /// Mandatory, not decorative: phase 2 explains a score with it, and a score
    /// computed from a three-week-old database means something different from
    /// the same score computed today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_snapshot_date: Option<String>,
    /// Unix milliseconds.
    pub scanned_at: u64,
}

// ===== Scanner metadata =====

#[derive(Debug, Clone, Deserialize)]
struct TrivyVersion {
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "VulnerabilityDB")]
    vulnerability_db: Option<TrivyDbInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct TrivyDbInfo {
    #[serde(rename = "UpdatedAt")]
    updated_at: Option<String>,
}

/// Scanner version and database date, read from `trivy version --format json`.
///
/// The scan output itself carries neither, and both belong in every result: they
/// are what makes a finding list reproducible and a later score explainable.
fn scanner_metadata() -> (String, Option<String>) {
    let mut cmd = std::process::Command::new(crate::path_util::resolve_binary("trivy"));
    cmd.args(["version", "--format", "json"]);
    crate::path_util::apply_path_to_cmd(&mut cmd);

    let Ok(out) = cmd.output() else {
        return ("unknown".to_string(), None);
    };
    let parsed: Option<TrivyVersion> = serde_json::from_slice(&out.stdout).ok();
    match parsed {
        Some(v) => (
            v.version.unwrap_or_else(|| "unknown".to_string()),
            v.vulnerability_db.and_then(|db| db.updated_at),
        ),
        None => ("unknown".to_string(), None),
    }
}

// ===== Trivy JSON =====

#[derive(Debug, Deserialize)]
struct TrivyReport {
    #[serde(rename = "Results")]
    results: Option<Vec<TrivyResult>>,
}

#[derive(Debug, Deserialize)]
struct TrivyResult {
    #[serde(rename = "Vulnerabilities")]
    vulnerabilities: Option<Vec<TrivyVuln>>,
}

/// Every field optional except the id.
///
/// The parser has to be forgiving in one direction only: an unknown field is
/// ignored and a missing one becomes `Unknown`, but a finding is never dropped.
/// A scanner upgrade that reshapes its JSON must not silently empty a report —
/// "no vulnerabilities" and "I could not read the answer" look identical to a
/// user and mean opposite things.
#[derive(Debug, Deserialize)]
struct TrivyVuln {
    #[serde(rename = "VulnerabilityID")]
    id: Option<String>,
    #[serde(rename = "PkgName")]
    pkg_name: Option<String>,
    #[serde(rename = "InstalledVersion")]
    installed_version: Option<String>,
    #[serde(rename = "FixedVersion")]
    fixed_version: Option<String>,
    #[serde(rename = "Severity")]
    severity: Option<String>,
    #[serde(rename = "DataSource")]
    data_source: Option<TrivyDataSource>,
    #[serde(rename = "CVSS")]
    cvss: Option<HashMap<String, TrivyCvss>>,
    #[serde(rename = "PublishedDate")]
    published: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrivyDataSource {
    #[serde(rename = "ID")]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrivyCvss {
    #[serde(rename = "V3Score")]
    v3_score: Option<f64>,
    #[serde(rename = "V2Score")]
    v2_score: Option<f64>,
}

/// One score out of the per-vendor table, chosen the same way every time.
///
/// Trivy reports CVSS per scoring vendor (`nvd`, `redhat`, `ghsa`, …) in a map,
/// and iteration order over a map is not stable — taking "the first one with a
/// score" made the same report yield 9.8 on one run and 7.5 on the next, which
/// phase 2 would then turn into a score that moves without the image changing.
///
/// NVD first because it is the reference scoring; otherwise the highest v3 on
/// offer, and only then the highest v2, because a v2-only advisory is old rather
/// than mild.
fn pick_cvss(scores: Option<&HashMap<String, TrivyCvss>>) -> Option<f64> {
    let scores = scores?;
    if let Some(nvd) = scores.get("nvd").and_then(|c| c.v3_score.or(c.v2_score)) {
        return Some(nvd);
    }
    let highest = |pick: fn(&TrivyCvss) -> Option<f64>| {
        scores
            .values()
            .filter_map(pick)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    };
    highest(|c| c.v3_score).or_else(|| highest(|c| c.v2_score))
}

/// Parse a Trivy report into normalised findings.
///
/// Public to the crate so tests can drive it from frozen fixtures rather than
/// from a scanner that has to be installed and has a database that changes.
pub fn parse_trivy_report(json: &str) -> Result<Vec<Finding>, String> {
    let report: TrivyReport =
        serde_json::from_str(json).map_err(|e| format!("Cannot read scanner output: {}", e))?;

    let mut findings = Vec::new();
    for result in report.results.unwrap_or_default() {
        for v in result.vulnerabilities.unwrap_or_default() {
            findings.push(Finding {
                // An entry with no id is still an entry. Naming it "unknown"
                // keeps it countable and visible; dropping it would quietly
                // reduce a vulnerability count, which is the one number here
                // nobody should have to distrust.
                id: v.id.unwrap_or_else(|| "unknown".to_string()),
                package: v.pkg_name.unwrap_or_else(|| "unknown".to_string()),
                installed_version: v.installed_version.unwrap_or_default(),
                fixed_version: v.fixed_version.filter(|s| !s.is_empty()),
                severity: v.severity.as_deref().map_or(Severity::Unknown, Severity::parse),
                source: v.data_source.and_then(|d| d.id),
                cvss: pick_cvss(v.cvss.as_ref()),
                published: v.published,
            });
        }
    }
    Ok(findings)
}

// ===== Cache =====

/// How long a result is reused.
///
/// Long, because the image is immutable: the same digest yields the same
/// packages forever. What does change is the vulnerability database, which is
/// why the TTL exists at all rather than the cache being permanent.
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

struct CacheEntry {
    result: ScanResult,
    stored_at: Instant,
}

static CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached(digest: &str) -> Option<ScanResult> {
    let guard = CACHE.lock().ok()?;
    let entry = guard.get(digest)?;
    (entry.stored_at.elapsed() < CACHE_TTL).then(|| entry.result.clone())
}

fn store(result: &ScanResult) {
    if let Ok(mut guard) = CACHE.lock() {
        // Expired entries are dropped here rather than never: a long-running app
        // that scans many images would otherwise hold every finding list it has
        // ever produced, and a fat image carries thousands.
        guard.retain(|_, e| e.stored_at.elapsed() < CACHE_TTL);
        guard.insert(
            result.image_digest.clone(),
            CacheEntry {
                result: result.clone(),
                stored_at: Instant::now(),
            },
        );
    }
}

/// Forget cached results. Called when the database is updated or the user asks
/// for a fresh scan.
pub fn invalidate() {
    if let Ok(mut guard) = CACHE.lock() {
        guard.clear();
    }
}

// ===== Input validation =====

/// Accept an image reference, refuse an argument.
///
/// The reference is positional in the scanner's argv, so one starting with `-`
/// changes what the command means.
fn require_image_ref(image_ref: &str) -> Result<(), String> {
    let trimmed = image_ref.trim();
    if trimmed.is_empty() {
        return Err("Image reference must not be empty".to_string());
    }
    if trimmed.starts_with('-') {
        return Err(format!("Invalid image reference: {:?}", image_ref));
    }
    // The charset `validation::is_valid_container_id` accepts, plus `@` for a
    // digest reference. Spaces and shell metacharacters are absent from both.
    let body_ok = trimmed.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@')
    });
    if !body_ok || trimmed.len() > 256 {
        return Err(format!("Invalid image reference: {:?}", image_ref));
    }
    Ok(())
}

/// Scan ids come from the caller so a cancel can be armed before the process
/// exists — the same gap `streaming_cmd::arm_cancel` was written for.
fn require_scan_id(scan_id: &str) -> Result<(), String> {
    let ok = !scan_id.is_empty()
        && scan_id.len() <= 64
        && scan_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid scan id: {:?}", scan_id))
    }
}

// ===== Scanning =====

fn emit(event: &str, payload: serde_json::Value) {
    crate::sse::publish_sse_event(event, &payload);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The image's content id, via the container runtime.
///
/// Doubles as the "is this image here?" check. Pulling is deliberately not
/// attempted: it costs bandwidth the user has not agreed to spend, and a missing
/// image is something they can fix in one click elsewhere in the app.
fn image_digest(image_ref: &str) -> Result<String, String> {
    let mut cmd = crate::commands::runtime::get_runtime_cmd();
    cmd.args(["image", "inspect", "--format", "{{.Id}}", "--", image_ref]);
    let out = cmd
        .output()
        .map_err(|e| crate::redact::redact_err("Cannot inspect image", e))?;
    if !out.status.success() {
        return Err(format!(
            "Image {} is not present locally. Pull it first, then scan.",
            image_ref
        ));
    }
    let digest = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if digest.is_empty() {
        return Err(format!("Could not determine the image id for {}", image_ref));
    }
    Ok(digest)
}

/// Local sources only, in priority order.
///
/// Trivy's default is `docker,containerd,podman,remote`, and that last entry is
/// the problem: if the local daemon lookup fails for any reason — the socket is
/// not visible to this process, the image was deleted between listing and
/// scanning — Trivy quietly resolves the reference against a public registry and
/// **sends the image name off the machine**. The app promises that never
/// happens, so the promise is enforced here rather than written in the UI.
///
/// This is the same class of defect that ruled Grype out during the phase 1
/// bakeoff; Trivy avoids it by default configuration, not by construction.
const LOCAL_IMAGE_SOURCES: &str = "docker,containerd,podman";

fn trivy_cmd(args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new(crate::path_util::resolve_binary("trivy"));
    cmd.args(args);
    crate::path_util::apply_path_to_cmd(&mut cmd);
    cmd
}

/// Download the vulnerability database if it is stale, as a separate step.
///
/// Folded into the scan it would be an unexplained minute of silence on first
/// run — the database is 1.2 GB, and the bakeoff measured 41.6s to fetch it on a
/// fast connection. Split out, the UI can say which of the two things is
/// happening, and the scan itself then runs with `--skip-db-update`, which is
/// also what makes it work offline.
///
/// A failure here is not fatal: an existing database still scans. Only a first
/// run with no database at all will then fail, and it fails in the scan step
/// with the scanner's own message.
///
/// Returns true when the user cancelled during this stage — the scan must not
/// then start, or stopping the download would be followed by the very work it
/// was downloading for.
fn update_db(scan_id: &str) -> bool {
    emit(
        "security-scan-progress",
        serde_json::json!({ "scanId": scan_id, "stage": "database", "bytes": 0 }),
    );
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    // Drained on another thread: `ByLine` blocks the writer once the channel
    // buffer backs up, and nothing here wants the scanner's chatter.
    std::thread::spawn(move || while rx.recv().is_ok() {});
    let before = scanner_metadata().1;

    let cancelled = streaming_cmd::run_streaming(
        &db_job_id(scan_id),
        trivy_cmd(&["image", "--download-db-only"]),
        "trivy",
        OutputSink::ByLine(tx),
        |_| {},
    )
    .map(|outcome| outcome.cancelled)
    .unwrap_or(false);

    // A newer database can turn a clean image into a vulnerable one without the
    // image changing at all, so every cached result is now answering a question
    // that has moved. Only on an actual change: clearing on every scan would
    // make the cache do nothing.
    if !cancelled && scanner_metadata().1 != before {
        invalidate();
    }
    cancelled
}

/// Scan one image.
///
/// Blocking: call it from `helpers::run_blocking`. Progress is published to SSE
/// under `scan_id`, and the same id cancels it.
pub fn scan_image_blocking(scan_id: &str, image_ref: &str, refresh: bool) -> Result<ScanResult, String> {
    require_scan_id(scan_id)?;
    require_image_ref(image_ref)?;

    let digest = image_digest(image_ref)?;
    if !refresh {
        if let Some(hit) = cached(&digest) {
            return Ok(hit);
        }
    }

    // From here on there are child processes to cancel, so the armed-cancel
    // bookkeeping has to be cleaned up however this function leaves.
    let _guard = CancelGuard { scan_id: scan_id.to_string() };

    if update_db(scan_id) {
        return Err("Scan cancelled".to_string());
    }

    emit(
        "security-scan-progress",
        serde_json::json!({ "scanId": scan_id, "stage": "scan", "bytes": 0 }),
    );

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    // The report is JSON on stdout; collect it while the process runs rather
    // than after it, so a cancel takes effect immediately and a large report
    // never sits in a pipe buffer waiting for a reader.
    let collector = std::thread::spawn(move || {
        let mut buf = String::new();
        while let Ok(line) = rx.recv() {
            buf.push_str(&line);
            buf.push('\n');
        }
        buf
    });

    let outcome = streaming_cmd::run_streaming(
        scan_id,
        trivy_cmd(&[
            "image",
            "--skip-db-update",
            // Never `remote`: see LOCAL_IMAGE_SOURCES.
            "--image-src",
            LOCAL_IMAGE_SOURCES,
            "--format",
            "json",
            // Vulnerabilities only. Trivy also scans for secrets by default,
            // which reads file *contents* out of the image — more time and a
            // much wider blast radius than the question this phase asks.
            "--scanners",
            "vuln",
            "--",
            image_ref,
        ]),
        "trivy",
        OutputSink::ByLine(tx),
        |bytes| {
            emit(
                "security-scan-progress",
                serde_json::json!({ "scanId": scan_id, "stage": "scan", "bytes": bytes }),
            );
        },
    );

    let json = collector.join().unwrap_or_default();

    let outcome = outcome.map_err(|e| crate::redact::redact(&e))?;
    if outcome.cancelled {
        return Err("Scan cancelled".to_string());
    }

    let findings = parse_trivy_report(&json)?;
    let (scanner_version, db_snapshot_date) = scanner_metadata();

    let result = ScanResult {
        image_ref: image_ref.to_string(),
        image_digest: digest,
        findings,
        scanner: ScannerKind::Trivy,
        scanner_version,
        db_snapshot_date,
        scanned_at: now_ms(),
    };
    store(&result);
    Ok(result)
}

/// Write an SBOM for `image_ref` to `dest_path`.
///
/// The destination is confined the same way every other written path in this app
/// is: resolved against the folder the caller named, so a `..` in the file name
/// is a rejection rather than a tautology.
pub fn export_sbom_blocking(
    image_ref: &str,
    dest_dir: &str,
    file_name: &str,
    format: SbomFormat,
    overwrite: bool,
) -> Result<String, String> {
    require_image_ref(image_ref)?;
    require_plain("Destination folder", dest_dir)?;
    require_plain("File name", file_name)?;

    let base = std::path::Path::new(dest_dir);
    if !base.is_dir() {
        return Err(format!("Destination folder does not exist: {}", dest_dir));
    }
    let dest = base.join(file_name);
    // The base is the folder the caller named, not the candidate's own parent —
    // that is what makes `../` in the file name a rejection rather than a
    // tautology. Same rule as `commands::file_transfer`.
    crate::validation::assert_path_within(base, &dest)?;

    if dest.exists() && !overwrite {
        return Err(format!(
            "{} already exists. Choose another name or confirm overwriting.",
            dest.display()
        ));
    }

    // Written to a scratch file and renamed, so a failed or killed export cannot
    // leave a truncated SBOM at the path the user chose. A half-written document
    // that looks complete is worse than no document.
    let scratch = base.join(format!(".{}.part", file_name));
    crate::validation::assert_path_within(base, &scratch)?;
    let _ = std::fs::remove_file(&scratch);

    let out = trivy_cmd(&[
        "image",
        "--skip-db-update",
        "--image-src",
        LOCAL_IMAGE_SOURCES,
        "--format",
        format.as_trivy_arg(),
        "--output",
        &scratch.to_string_lossy(),
        "--",
        image_ref,
    ])
    .output()
    .map_err(|e| crate::redact::redact_err("Cannot run the scanner", e))?;

    if !out.status.success() {
        let _ = std::fs::remove_file(&scratch);
        return Err(crate::redact::redact(&String::from_utf8_lossy(&out.stderr)));
    }

    std::fs::rename(&scratch, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&scratch);
        crate::redact::redact_err("Cannot write the SBOM", e)
    })?;
    Ok(dest.to_string_lossy().to_string())
}

/// Reject a path component that is empty, carries control characters, or would
/// be read as a flag. Mirrors `commands::file_transfer::require_plain`.
fn require_plain(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{} must not be empty", label));
    }
    if value.chars().any(|c| c.is_control()) {
        return Err(format!("{} contains control characters", label));
    }
    if value.trim_start().starts_with('-') {
        return Err(format!("{} must not start with '-'", label));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomFormat {
    CycloneDx,
    Spdx,
}

impl SbomFormat {
    fn as_trivy_arg(self) -> &'static str {
        match self {
            SbomFormat::CycloneDx => "cyclonedx",
            SbomFormat::Spdx => "spdx-json",
        }
    }
}

// ===== Commands =====

#[tauri::command]
pub async fn security_scan_image(
    scan_id: String,
    image_ref: String,
    refresh: Option<bool>,
) -> Result<ScanResult, ColimaError> {
    // Validated here so bad input is reported as bad input. Inside the blocking
    // body every failure looks the same to the caller, and "invalid image
    // reference" is not a command failure.
    require_scan_id(&scan_id).map_err(ColimaError::validation)?;
    require_image_ref(&image_ref).map_err(ColimaError::validation)?;

    let refresh = refresh.unwrap_or(false);
    crate::helpers::run_blocking(move || scan_image_blocking(&scan_id, &image_ref, refresh))
        .await
        .map_err(ColimaError::command_failed)
}

/// Stop a running scan. Also arms the cancel for a scan whose process has not
/// been spawned yet, which is the gap a user clicking quickly lands in.
///
/// Both stages are cancelled. The database download is the long one — it is the
/// stage a user is most likely to give up on — and cancelling only the scan
/// would leave 1.2 GB still coming down after they stopped it.
///
/// Returns whether a process was actually killed; an armed-but-not-yet-started
/// scan reports `false` and still stops.
pub fn cancel(scan_id: &str) -> Result<bool, String> {
    require_scan_id(scan_id)?;
    let db_killed = streaming_cmd::arm_cancel(&db_job_id(scan_id));
    let scan_killed = streaming_cmd::arm_cancel(scan_id);
    Ok(db_killed || scan_killed)
}

/// The database stage runs as its own job so it can be cancelled and reported
/// separately; its id is derived rather than passed so the two cannot drift.
fn db_job_id(scan_id: &str) -> String {
    format!("{}-db", scan_id)
}

/// Clears both armed cancels when the scan leaves, however it leaves.
///
/// An armed cancel that is never consumed stays in the pending set forever, and
/// scan ids come from the caller — so a cancel arriving just after a scan
/// finished would kill the *next* scan reusing that id, at birth, with nothing
/// to explain it. `Drop` rather than calls on each return path: this function
/// has several, including `?`.
struct CancelGuard {
    scan_id: String,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        streaming_cmd::disarm_cancel(&db_job_id(&self.scan_id));
        streaming_cmd::disarm_cancel(&self.scan_id);
    }
}

#[tauri::command]
pub async fn security_scan_cancel(scan_id: String) -> Result<bool, ColimaError> {
    cancel(&scan_id).map_err(ColimaError::validation)
}

/// Scan an image, evaluate the configuration rules against it, and score both.
///
/// One call rather than three: a score is only meaningful next to the findings
/// and rule results it came from, and letting a caller assemble it from separate
/// requests is how a score ends up displayed beside a different image's scan.
pub fn audit_image_blocking(
    scan_id: &str,
    image_ref: &str,
    level: crate::commands::security_rules::Level,
    refresh: bool,
    now_ms: i64,
) -> Result<SecurityAudit, String> {
    let scan = scan_image_blocking(scan_id, image_ref, refresh)?;
    let facts = crate::commands::security_rules::collect_facts_blocking(image_ref)?;
    let evaluation = crate::commands::security_rules::evaluate(&facts, level, now_ms);
    let score = crate::commands::security_score::score(&scan, &evaluation, level);
    let audit = SecurityAudit { scan, evaluation, score };

    Ok(audit)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityAudit {
    pub scan: ScanResult,
    pub evaluation: crate::commands::security_rules::Evaluation,
    pub score: crate::commands::security_score::ScoreBreakdown,
}

#[tauri::command]
pub async fn security_audit_image(
    scan_id: String,
    image_ref: String,
    level: Option<crate::commands::security_rules::Level>,
    refresh: Option<bool>,
) -> Result<SecurityAudit, ColimaError> {
    require_scan_id(&scan_id).map_err(ColimaError::validation)?;
    require_image_ref(&image_ref).map_err(ColimaError::validation)?;

    let level = level.unwrap_or_default();
    let refresh = refresh.unwrap_or(false);
    // The clock is read here, at the edge, and passed in — so everything below
    // is reproducible from a stored result.
    let now = now_ms() as i64;
    crate::helpers::run_blocking(move || {
        audit_image_blocking(&scan_id, &image_ref, level, refresh, now)
    })
    .await
    .map_err(ColimaError::command_failed)
}

#[tauri::command]
pub async fn security_sbom_export(
    image_ref: String,
    dest_dir: String,
    file_name: String,
    format: SbomFormat,
    overwrite: Option<bool>,
) -> Result<String, ColimaError> {
    let overwrite = overwrite.unwrap_or(false);
    crate::helpers::run_blocking(move || {
        export_sbom_blocking(&image_ref, &dest_dir, &file_name, format, overwrite)
    })
    .await
    .map_err(ColimaError::command_failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{}", name))
            .unwrap_or_else(|e| panic!("fixture {}: {}", name, e))
    }

    #[test]
    fn severity_is_normalised_across_scanner_spellings() {
        // Trivy shouts, Grype does not, and some advisories say "Important".
        assert_eq!(Severity::parse("CRITICAL"), Severity::Critical);
        assert_eq!(Severity::parse("Critical"), Severity::Critical);
        assert_eq!(Severity::parse("Important"), Severity::High);
        assert_eq!(Severity::parse("negligible"), Severity::Low);
    }

    #[test]
    fn an_unknown_severity_is_kept_as_a_finding() {
        // Dropping it would hide a vulnerability because a label was new.
        assert_eq!(Severity::parse("spicy"), Severity::Unknown);
    }

    #[test]
    fn a_clean_image_parses_as_zero_findings() {
        let findings = parse_trivy_report(&fixture("trivy-clean.json")).expect("parse");
        assert!(findings.is_empty());
    }

    #[test]
    fn findings_are_flattened_across_targets() {
        let findings = parse_trivy_report(&fixture("trivy-mixed.json")).expect("parse");
        assert_eq!(findings.len(), 3);

        let critical = findings.iter().find(|f| f.id == "CVE-2024-0001").expect("cve");
        assert_eq!(critical.severity, Severity::Critical);
        assert_eq!(critical.package, "openssl");
        assert_eq!(critical.fixed_version.as_deref(), Some("3.0.13-r0"));
        assert_eq!(critical.source.as_deref(), Some("alpine"));
        assert_eq!(critical.cvss, Some(9.8));

        // No fix published: distinct from an empty string, because it decides
        // whether the user can do anything about it.
        let unfixed = findings.iter().find(|f| f.id == "CVE-2024-0002").expect("cve");
        assert!(unfixed.fixed_version.is_none());
    }

    #[test]
    fn unknown_fields_and_missing_ones_do_not_lose_findings() {
        // A scanner upgrade that adds fields, drops optional ones, or invents a
        // severity must not empty the report — "nothing found" and "could not
        // read" mean opposite things to the person reading it.
        let findings = parse_trivy_report(&fixture("trivy-future.json")).expect("parse");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].severity, Severity::Unknown);
        assert_eq!(findings[0].installed_version, "");
        assert_eq!(findings[1].package, "unknown");
    }

    #[test]
    fn malformed_output_is_an_error_not_an_empty_report() {
        assert!(parse_trivy_report("not json at all").is_err());
    }

    #[test]
    fn image_references_that_would_read_as_flags_are_refused() {
        assert!(require_image_ref("nginx:latest").is_ok());
        assert!(require_image_ref("ghcr.io/org/app@sha256:abc").is_ok());
        assert!(require_image_ref("--config=/tmp/evil").is_err());
        assert!(require_image_ref("nginx latest").is_err());
        assert!(require_image_ref("").is_err());
    }

    #[test]
    fn scan_ids_are_constrained() {
        assert!(require_scan_id("scan-1_A").is_ok());
        assert!(require_scan_id("../etc").is_err());
        assert!(require_scan_id("").is_err());
    }

    fn result(tag: &str, digest: &str) -> ScanResult {
        ScanResult {
            image_ref: tag.into(),
            image_digest: digest.into(),
            findings: vec![],
            scanner: ScannerKind::Trivy,
            scanner_version: "0.73.0".into(),
            db_snapshot_date: None,
            scanned_at: 1,
        }
    }

    #[test]
    fn the_cache_is_keyed_by_digest_not_by_tag() {
        invalidate();
        store(&result("myapp:latest", "sha256:aaa"));
        assert!(cached("sha256:aaa").is_some());

        // The image is rebuilt under the same tag. A tag-keyed cache would hand
        // back the old image's findings, and nobody would notice until it
        // mattered — so the new build must miss, then store separately.
        assert!(cached("sha256:bbb").is_none());
        store(&result("myapp:latest", "sha256:bbb"));
        assert!(cached("sha256:aaa").is_some(), "the old image is still cached");
        assert!(cached("sha256:bbb").is_some());
        invalidate();
        assert!(cached("sha256:aaa").is_none());
    }

    #[test]
    fn cvss_is_the_same_number_on_every_run() {
        // Trivy reports one score per vendor in a map, and map iteration order
        // is not stable — picking "the first with a score" made the same report
        // yield different numbers between runs.
        let json = fixture("trivy-multi-cvss.json");
        let first = parse_trivy_report(&json).expect("parse");
        for _ in 0..20 {
            let again = parse_trivy_report(&json).expect("parse");
            assert_eq!(first[0].cvss, again[0].cvss);
        }
        // NVD is the reference scoring, so it wins over a vendor's own number.
        assert_eq!(first[0].cvss, Some(9.8));
        // No NVD entry: the highest v3 on offer rather than an arbitrary one.
        assert_eq!(first[1].cvss, Some(8.1));
        // v2-only advisories are old, not mild — still shown.
        assert_eq!(first[2].cvss, Some(6.4));
    }

    #[test]
    fn a_finding_without_an_id_is_still_counted() {
        let findings = parse_trivy_report(&fixture("trivy-future.json")).expect("parse");
        assert!(
            findings.iter().any(|f| f.id == "unknown"),
            "dropping it would quietly lower a vulnerability count"
        );
    }

    #[test]
    fn sbom_export_refuses_to_escape_the_chosen_folder() {
        let dir = std::env::temp_dir().join("colimaui-sbom-test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let dir_str = dir.to_string_lossy().to_string();

        for bad in ["../escape.json", "-o", ""] {
            assert!(
                export_sbom_blocking("nginx:alpine", &dir_str, bad, SbomFormat::Spdx, false)
                    .is_err(),
                "{:?} must be refused before the scanner is ever run",
                bad
            );
        }

        // An existing destination is refused unless overwriting is explicit.
        let existing = dir.join("taken.json");
        std::fs::write(&existing, b"{}").expect("write");
        let err = export_sbom_blocking("nginx:alpine", &dir_str, "taken.json", SbomFormat::Spdx, false)
            .expect_err("must refuse");
        assert!(err.contains("already exists"), "got: {}", err);

        std::fs::remove_dir_all(&dir).ok();
    }
}
