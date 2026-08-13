//! The diagnostic bundle behind "Report a problem".
//!
//! Everything here is assembled so the user can read it before deciding to share
//! it. Nothing is transmitted: there is no send path in this module, and the UI
//! offers only "copy", "save" and "open GitHub".
//!
//! ## Redaction is structural, not a step
//!
//! [`Section::new`] is the only way a section is built, and it redacts. That is
//! deliberate: a bundle is pasted into a public issue, so "remember to redact
//! before display" is one forgotten call away from leaking a password. Redacting
//! at construction means a section that skipped it cannot exist.
//!
//! Container **environment variables are never collected at all**. Logs are, and
//! logs do print secrets, which is why every line goes through `redact`.
//!
//! ## Partial failure is normal
//!
//! Colima may not be installed; the daemon may be down; there may be no crash to
//! report. Each collector therefore records its own failure inside its section
//! rather than failing the bundle — a report that is missing one section is
//! useful, and one that refuses to generate is not.

use serde::{Deserialize, Serialize};

use crate::redact::redact;

/// Log lines collected per container, unless the caller asks for fewer.
const DEFAULT_LOG_LINES: u32 = 200;

/// Ceiling on the requested line count.
///
/// `redact` costs roughly 0.8 s per megabyte in release and ten times that in
/// debug, so the size of what reaches it is a latency budget, not just a byte
/// budget. Clamping here keeps a caller asking for a million lines from turning
/// a bug report into a hang.
const MAX_LOG_LINES: u32 = 5_000;

/// Ceiling on raw log bytes, applied *before* redaction.
///
/// Trimming afterwards would mean paying to redact text that is then thrown
/// away — the expensive half of the work, done for nothing.
const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;

/// Hard ceiling on the whole bundle.
///
/// Large enough for real logs, small enough to paste. Logs are trimmed from the
/// oldest end when it is exceeded — the tail is where the failure is.
const MAX_BUNDLE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub title: String,
    /// Always redacted: see [`Section::new`].
    pub content: String,
    /// Sections holding logs start unchecked — they are the largest and the most
    /// likely to carry something the user would rather not publish.
    pub included_by_default: bool,
}

impl Section {
    /// Build a section, redacting its content.
    ///
    /// The single construction point for a reason: every collector goes through
    /// it, so a section that skipped redaction cannot exist. `render_markdown`
    /// redacts again on the way out, because the bundle round-trips through the
    /// client before being written.
    fn new(id: &str, title: &str, content: impl Into<String>, included_by_default: bool) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            content: redact(&content.into()),
            included_by_default,
        }
    }

    /// A section whose collector failed.
    ///
    /// Recorded rather than dropped: "colima is not installed" is often the
    /// answer to the bug being reported.
    fn unavailable(id: &str, title: &str, reason: impl std::fmt::Display) -> Self {
        Self::new(id, title, format!("(could not collect: {})", reason), true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundle {
    pub sections: Vec<Section>,
    /// Stable across machines for the same underlying error, so duplicate reports
    /// can be grouped. Empty when the user did not start from an error.
    pub signature: String,
    pub app_version: String,
    /// Bytes dropped to stay under the size cap. Non-zero means logs were cut.
    pub truncated_bytes: usize,
}

fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ===== Collectors =====
//
// One function per section. Each returns a `Section` and never an error: a
// collector that cannot do its job says so in its own content.

fn section_app() -> Section {
    let platform = crate::platform::detect_platform();
    Section::new(
        "app",
        "Application",
        format!(
            "ColimaUI {}\nOS: {}\nArch: {}\nWSL: {}",
            app_version(),
            platform.os,
            platform.arch,
            platform.wsl,
        ),
        true,
    )
}

async fn section_versions() -> Section {
    match crate::commands::system::check_system().await {
        Ok(info) => Section::new(
            "versions",
            "Tool versions",
            format!(
                "colima: {}\ndocker: {}\nlima: {}",
                if info.colima_installed { info.colima_version.trim() } else { "not installed" },
                if info.docker_installed { info.docker_version.trim() } else { "not installed" },
                if info.lima_installed { info.lima_version.trim() } else { "not installed" },
            ),
            true,
        ),
        Err(e) => Section::unavailable("versions", "Tool versions", e),
    }
}

fn section_host() -> Section {
    let specs = crate::commands::system::detect_host_specs();
    Section::new(
        "host",
        "Host",
        format!(
            "Model: {}\nArch: {}\nCPU cores: {}\nMemory: {} GiB\nDisk: {} GiB free of {} GiB",
            specs.model, specs.arch, specs.cpu_cores, specs.memory_gib, specs.disk_free_gib,
            specs.disk_total_gib,
        ),
        true,
    )
}

async fn section_instances() -> Section {
    match crate::commands::colima::list_instances().await {
        Ok(list) if list.is_empty() => {
            Section::new("instances", "Colima instances", "(none)", true)
        }
        Ok(list) => {
            let body = list
                .iter()
                .map(|i| {
                    format!(
                        "{} — status: {}, cpus: {}, memory: {}, disk: {}, runtime: {}",
                        i.name, i.status, i.cpus, i.memory, i.disk, i.runtime
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Section::new("instances", "Colima instances", body, true)
        }
        Err(e) => Section::unavailable("instances", "Colima instances", e),
    }
}

/// Running containers, by name and image only.
///
/// Deliberately not `docker inspect`: that carries the environment, which is
/// where passwords live. Nothing here needs it.
async fn section_containers() -> Section {
    match crate::commands::containers::list_containers_cli(false).await {
        Ok(list) if list.is_empty() => {
            Section::new("containers", "Running containers", "(none)", true)
        }
        Ok(list) => {
            let body = list
                .iter()
                .map(|c| {
                    let get = |k: &str| {
                        c.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
                    };
                    format!("{} — image: {}, status: {}", get("Names"), get("Image"), get("Status"))
                })
                .collect::<Vec<_>>()
                .join("\n");
            Section::new("containers", "Running containers", body, true)
        }
        Err(e) => Section::unavailable("containers", "Running containers", e),
    }
}

/// Logs for one container, when the report is about a specific one.
///
/// Off by default: logs are the biggest section and the one most likely to hold
/// something the user would not choose to publish, even after redaction.
async fn section_logs(container_id: &str, lines: u32) -> Section {
    let title = format!("Logs — {}", container_id);
    let lines = lines.min(MAX_LOG_LINES);
    match crate::commands::containers::container_logs(container_id.to_string(), lines).await {
        Ok(logs) => {
            let (logs, dropped) = trim_to_tail(&logs, MAX_LOG_BYTES);
            let body = if dropped > 0 {
                format!(
                    "(trimmed {} bytes of older log lines)\n{}",
                    dropped, logs
                )
            } else {
                logs
            };
            // Redaction happens here, on a bounded string, and off the async
            // worker: it is pure CPU work measured in seconds per megabyte.
            tokio::task::spawn_blocking(move || Section::new("logs", &title, body, false))
                .await
                .unwrap_or_else(|e| Section::unavailable("logs", "Logs", e))
        }
        Err(e) => {
            let mut s = Section::unavailable("logs", &title, e);
            s.included_by_default = false;
            s
        }
    }
}

fn section_crash() -> Section {
    match crate::crash::latest_report() {
        Some(report) => Section::new(
            "crash",
            "Most recent crash",
            format!("Recorded at unix {}\n\n{}", report.ts, report.content),
            true,
        ),
        None => Section::new("crash", "Most recent crash", "(no crash recorded)", true),
    }
}

// ===== Assembly =====

/// Total serialized size of the sections' contents.
fn bundle_size(sections: &[Section]) -> usize {
    sections.iter().map(|s| s.content.len()).sum()
}

/// Keep the tail of `text` within `max_bytes`, dropping whole lines from the
/// front. Returns the kept text and how many bytes went.
///
/// The tail is what matters: a failure is at the end of a log, not the start.
fn trim_to_tail(text: &str, max_bytes: usize) -> (String, usize) {
    if text.len() <= max_bytes {
        return (text.to_string(), 0);
    }
    let mut kept: Vec<&str> = text.lines().collect();
    let mut dropped = 0;
    let mut size = text.len();
    while size > max_bytes && !kept.is_empty() {
        let line = kept.remove(0);
        let cost = line.len() + 1;
        dropped += cost;
        size = size.saturating_sub(cost);
    }
    (kept.join("\n"), dropped)
}

/// Final guard on the assembled bundle.
///
/// Log sections are already bounded at collection, so this rarely fires; it
/// exists for the case of several log sections adding up. Only logs are cut —
/// the version and host sections are tiny and are exactly what makes a report
/// actionable, so shrinking those to fit would trade the useful part for the
/// bulky one.
///
/// Returns how many bytes were removed.
fn enforce_size_limit(sections: &mut [Section]) -> usize {
    let total = bundle_size(sections);
    if total <= MAX_BUNDLE_BYTES {
        return 0;
    }

    let mut excess = total - MAX_BUNDLE_BYTES;
    let mut removed = 0;

    for section in sections.iter_mut().filter(|s| s.id == "logs") {
        if excess == 0 {
            break;
        }
        let budget = section.content.len().saturating_sub(excess);
        let (kept, dropped) = trim_to_tail(&section.content, budget);
        if dropped == 0 {
            continue;
        }
        section.content = format!(
            "(trimmed {} bytes of older log lines to stay under the {} MB limit)\n{}",
            dropped,
            MAX_BUNDLE_BYTES / (1024 * 1024),
            kept
        );
        removed += dropped;
        excess = excess.saturating_sub(dropped);
    }

    removed
}

/// Build the bundle.
///
/// `error` is the message the user is reporting, if they started from one; it
/// only feeds the signature and is never stored as a section of its own.
pub async fn build_bundle(
    error: Option<String>,
    container_id: Option<String>,
    log_lines: Option<u32>,
) -> DiagnosticBundle {
    let mut sections = vec![
        section_app(),
        section_versions().await,
        section_host(),
        section_instances().await,
        section_containers().await,
        section_crash(),
    ];

    if let Some(id) = container_id.as_deref().filter(|s| !s.trim().is_empty()) {
        // Validated because it reaches `docker logs` as an argument; an invalid
        // id here should read as "no such container", not as a shell surprise.
        if crate::validation::is_valid_container_id(id) {
            sections.push(section_logs(id, log_lines.unwrap_or(DEFAULT_LOG_LINES)).await);
        } else {
            sections.push(Section::unavailable(
                "logs",
                "Logs",
                format!("invalid container id: {:?}", id),
            ));
        }
    }

    let truncated_bytes = enforce_size_limit(&mut sections);

    // Same algorithm the Knowledge Bank matches on, so a bug report and a stored
    // solution describe the same failure the same way.
    let signature = error
        .as_deref()
        .map(|e| crate::commands::compose_diagnose::error_signature(&redact(e)))
        .unwrap_or_default();

    DiagnosticBundle {
        sections,
        signature,
        app_version: app_version(),
        truncated_bytes,
    }
}

/// Render selected sections as Markdown, ready to paste into an issue.
///
/// Markdown rather than a zip: no archive dependency, it pastes directly into a
/// GitHub issue, and whoever reads the issue does not have to unpack anything.
///
/// The result is redacted once more before it is returned. The bundle reaches
/// this function having round-tripped through the client, so what is written to
/// disk is whatever came back over IPC; redacting again makes "anything this app
/// writes is redacted" true of the code path rather than of an assumption about
/// the caller. `redact` is idempotent — its mask characters are excluded from the
/// value patterns — so a second pass cannot damage an already-masked string.
pub fn render_markdown(bundle: &DiagnosticBundle, include: &[String]) -> String {
    redact(&render_markdown_raw(bundle, include))
}

fn render_markdown_raw(bundle: &DiagnosticBundle, include: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&format!("## ColimaUI diagnostics ({})\n\n", bundle.app_version));
    if !bundle.signature.is_empty() {
        out.push_str(&format!("**Signature:** `{}`\n\n", bundle.signature));
    }
    if bundle.truncated_bytes > 0 {
        out.push_str(&format!(
            "> {} bytes of older log lines were trimmed.\n\n",
            bundle.truncated_bytes
        ));
    }
    for section in bundle
        .sections
        .iter()
        .filter(|s| include.iter().any(|id| id == &s.id))
    {
        out.push_str(&format!("### {}\n\n```\n{}\n```\n\n", section.title, section.content));
    }
    out
}

// ===== Commands =====

#[tauri::command]
pub async fn diagnostic_bundle(
    error: Option<String>,
    container_id: Option<String>,
    log_lines: Option<u32>,
) -> Result<DiagnosticBundle, crate::error::ColimaError> {
    Ok(build_bundle(error, container_id, log_lines).await)
}

/// Write the selected sections to a file the user chose.
///
/// Path handling matches the transfer commands: the folder and the file name
/// arrive separately so containment is checked against the folder the user
/// picked, which a single joined path could not express.
#[tauri::command]
pub async fn save_diagnostic_bundle(
    bundle: DiagnosticBundle,
    include: Vec<String>,
    dest_dir: String,
    file_name: String,
    overwrite: bool,
) -> Result<String, crate::error::ColimaError> {
    Ok(save_bundle(&bundle, &include, &dest_dir, &file_name, overwrite)?)
}

pub fn save_bundle(
    bundle: &DiagnosticBundle,
    include: &[String],
    dest_dir: &str,
    file_name: &str,
    overwrite: bool,
) -> Result<String, String> {
    if dest_dir.trim().is_empty() || file_name.trim().is_empty() {
        return Err("Choose a folder and a file name".to_string());
    }
    let base = std::path::Path::new(dest_dir);
    if !base.is_dir() {
        return Err(format!("Destination folder does not exist: {}", dest_dir));
    }
    let target = base.join(file_name);
    crate::validation::assert_path_within(base, &target)?;
    if target.exists() && !overwrite {
        return Err(format!(
            "{} already exists. Choose another name or confirm overwriting.",
            target.display()
        ));
    }

    std::fs::write(&target, render_markdown(bundle, include))
        .map_err(|e| format!("Could not write {}: {}", target.display(), e))?;
    Ok(target.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_with(content: &str) -> DiagnosticBundle {
        DiagnosticBundle {
            sections: vec![Section::new("logs", "Logs", content, false)],
            signature: String::new(),
            app_version: "test".to_string(),
            truncated_bytes: 0,
        }
    }

    /// The fixture this phase exists to defend against. Every one of these has a
    /// path into a bundle: environment printed at container start, a token in a
    /// URL, a key echoed by a failing CLI.
    #[test]
    fn no_fixture_secret_survives_section_construction() {
        let leaky = concat!(
            "POSTGRES_PASSWORD=hunter2\n",
            "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n",
            "Authorization: Bearer sk-abcdefghijklmnopqrstuvwxyz0123456789\n",
            "https://api.example.com/v1?api_key=secret-value-here\n",
            "postgres://admin:s3cr3t@localhost:5432/app\n",
            "redis-server --requirepass topsecret\n",
            "{\"SUPABASE_SERVICE_ROLE_KEY\": \"eyJhbGciOiJIUzI1NiJ9.payload.sig\"}\n",
            "config at /Users/alice/.colima/default/colima.yaml\n",
        );
        let section = Section::new("logs", "Logs", leaky, false);

        for needle in [
            "hunter2",
            "AKIAIOSFODNN7EXAMPLE",
            "sk-abcdefghijklmnopqrstuvwxyz0123456789",
            "secret-value-here",
            "s3cr3t",
            "topsecret",
            "/Users/alice",
        ] {
            assert!(
                !section.content.contains(needle),
                "{:?} leaked into a section:\n{}",
                needle,
                section.content
            );
        }
    }

    #[test]
    fn a_postgres_password_in_a_container_log_is_masked() {
        // Called out separately because it is the exact scenario in the plan:
        // `docker logs` on a database container prints its environment at boot.
        let log = "2024-01-01 db  | POSTGRES_PASSWORD=hunter2\n2024-01-01 db  | ready";
        let section = Section::new("logs", "Logs", log, false);
        assert!(!section.content.contains("hunter2"));
        // The surrounding log must survive: over-redaction makes the report useless.
        assert!(section.content.contains("ready"));
    }

    #[test]
    fn home_paths_lose_the_account_name() {
        let section = Section::new("app", "App", "/Users/longnd/Desktop/project", true);
        assert!(!section.content.contains("longnd"));
        assert!(section.content.contains("/Users/<user>"));
    }

    #[test]
    fn a_failed_collector_becomes_a_section_rather_than_an_error() {
        let s = Section::unavailable("instances", "Colima instances", "colima is not installed");
        assert!(s.content.contains("could not collect"));
        assert!(s.content.contains("not installed"));
    }

    #[test]
    fn logs_are_not_included_by_default() {
        // The largest section and the likeliest to hold something private, even
        // after redaction. The user opts in.
        let s = Section::new("logs", "Logs", "anything", false);
        assert!(!s.included_by_default);
    }

    #[test]
    fn trim_to_tail_drops_whole_lines_from_the_front() {
        let text = "aaaa\nbbbb\ncccc\ndddd";
        let (kept, dropped) = trim_to_tail(text, 10);
        assert!(dropped > 0);
        assert!(kept.len() <= 10, "kept {} bytes", kept.len());
        // The newest line survives; the oldest is the one that goes.
        assert!(kept.ends_with("dddd"));
        assert!(!kept.contains("aaaa"));
        // A cut lands on a line boundary, never mid-token.
        assert!(kept.lines().all(|l| l.len() == 4));
    }

    #[test]
    fn trim_to_tail_leaves_short_text_untouched() {
        let (kept, dropped) = trim_to_tail("short", 1024);
        assert_eq!(kept, "short");
        assert_eq!(dropped, 0);
    }

    #[test]
    fn oversized_logs_are_trimmed_from_the_oldest_end() {
        // Sized against the guard rather than the 5 MB ceiling: redaction costs
        // seconds per megabyte, so a multi-megabyte fixture here would make the
        // test suite pay for a case the collector already prevents.
        let line = "x".repeat(99);
        let huge = (0..100).map(|i| format!("{i:04} {line}")).collect::<Vec<_>>().join("\n");
        let mut sections = vec![
            Section::new("app", "App", "small", true),
            Section::new("logs", "Logs", &huge, false),
        ];

        // Exercise the same code path the 5 MB ceiling uses.
        let budget = 2_000;
        let (kept, dropped) = trim_to_tail(&sections[1].content, budget);
        assert!(dropped > 0);
        assert!(kept.len() <= budget);
        assert!(kept.contains("0099 "), "the newest line was trimmed away");
        assert!(!kept.contains("0000 "), "the oldest line should be gone");

        // And the whole-bundle guard is a no-op well under the ceiling.
        assert_eq!(enforce_size_limit(&mut sections), 0);
    }

    #[test]
    fn a_bundle_under_the_limit_is_left_alone() {
        let mut sections = vec![Section::new("logs", "Logs", "short", false)];
        assert_eq!(enforce_size_limit(&mut sections), 0);
        assert_eq!(sections[0].content, "short");
    }

    #[test]
    fn the_same_error_signs_the_same_on_different_machines() {
        // Same failure, two machines: different home directory, different line
        // numbers. Grouping duplicate reports depends on these matching.
        let a = "yaml: line 12: mapping values are not allowed at /Users/alice/app/docker-compose.yml";
        let b = "yaml: line 47: mapping values are not allowed at /home/bob/srv/docker-compose.yml";
        let sig_a = crate::commands::compose_diagnose::error_signature(&redact(a));
        let sig_b = crate::commands::compose_diagnose::error_signature(&redact(b));
        assert_eq!(sig_a, sig_b, "\na: {}\nb: {}", sig_a, sig_b);
        assert!(!sig_a.is_empty());
    }

    #[test]
    fn markdown_renders_only_the_selected_sections() {
        let bundle = DiagnosticBundle {
            sections: vec![
                Section::new("app", "Application", "version info", true),
                Section::new("logs", "Logs", "log content", false),
            ],
            signature: "some failure".to_string(),
            app_version: "1.2.3".to_string(),
            truncated_bytes: 0,
        };

        let md = render_markdown(&bundle, &["app".to_string()]);
        assert!(md.contains("Application"));
        assert!(md.contains("version info"));
        assert!(md.contains("some failure"));
        // Unchecked sections must not appear — that checkbox is the whole point.
        assert!(!md.contains("log content"));
    }

    /// A bundle comes back from the client before being written, so the render
    /// step cannot assume its content is still the redacted text we produced.
    #[test]
    fn rendering_redacts_a_bundle_that_came_back_unredacted() {
        let tampered = DiagnosticBundle {
            // Built by hand, as a client could send it — not via Section::new.
            sections: vec![Section {
                id: "logs".to_string(),
                title: "Logs".to_string(),
                content: "POSTGRES_PASSWORD=hunter2".to_string(),
                included_by_default: false,
            }],
            signature: String::new(),
            app_version: "test".to_string(),
            truncated_bytes: 0,
        };
        let md = render_markdown(&tampered, &["logs".to_string()]);
        assert!(!md.contains("hunter2"), "unredacted content reached the output:\n{}", md);
    }

    #[test]
    fn redaction_is_idempotent() {
        // The second pass must not mangle what the first one masked.
        let once = redact("POSTGRES_PASSWORD=hunter2 and /Users/alice/x");
        assert_eq!(redact(&once), once);
    }

    #[test]
    fn markdown_states_that_logs_were_trimmed() {
        let mut bundle = bundle_with("kept");
        bundle.truncated_bytes = 4096;
        let md = render_markdown(&bundle, &["logs".to_string()]);
        assert!(md.contains("4096 bytes"));
    }

    #[test]
    fn saving_refuses_to_escape_the_chosen_folder() {
        let dir = std::env::temp_dir().join(format!(
            "colimaui-diag-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let bundle = bundle_with("content");

        let err = save_bundle(&bundle, &["logs".to_string()], dir.to_str().unwrap(), "../escaped.md", false)
            .expect_err("traversal must be refused");
        assert!(err.contains("escapes"), "unexpected error: {}", err);

        let path = save_bundle(&bundle, &["logs".to_string()], dir.to_str().unwrap(), "report.md", false)
            .expect("a plain name should write");
        assert!(std::fs::read_to_string(&path).unwrap().contains("content"));

        // And it will not silently replace an existing report.
        let err = save_bundle(&bundle, &["logs".to_string()], dir.to_str().unwrap(), "report.md", false)
            .expect_err("overwrite must be explicit");
        assert!(err.contains("already exists"));

        let _ = std::fs::remove_dir_all(&dir);
    }

}
