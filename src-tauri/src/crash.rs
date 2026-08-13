//! Crash reporting with mandatory redaction.
//!
//! A Rust panic carries a free-form message and, in a backtrace, arbitrary
//! strings from the call site. That is precisely where secrets leak: a panic in
//! an HTTP path can carry a request URL that includes an API key as a query
//! parameter (see the `redact` module for why `reqwest` does this). The default
//! panic hook prints that message straight to stderr.
//!
//! This installs a hook that routes the panic message through `redact()` before
//! it goes anywhere. Telemetry's closed event enum cannot cover crash text —
//! that is the whole reason crash reporting is a separate, redacted path.
//!
//! Reports are redacted, printed to stderr, and kept in `~/.colima-ui/crashes`
//! (newest 10). **Transmission is still deferred** — nothing is sent anywhere.
//! The diagnostic bundle reads the newest file only when the user asks it to,
//! and shows them the contents before they send anything.
//!
//! Persisting matters because stderr is invisible to anyone running the packaged
//! app: the crash that mattered happened yesterday, with no terminal attached.
//! Redaction happens *before* the write, so there is no path that stores a raw
//! panic message and redacts later.

use std::panic;

use crate::redact::redact;

/// Reduce a panic payload to a printable string. `panic!` payloads are usually
/// `&str` or `String`; anything else is reported opaquely rather than guessed.
fn payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Build the redacted, single-line report for a panic. Split out from the hook
/// so it can be tested without actually panicking.
pub fn redacted_report(location: Option<&str>, payload: &str) -> String {
    let raw = match location {
        Some(loc) => format!("panic at {loc}: {payload}"),
        None => format!("panic: {payload}"),
    };
    redact(&raw)
}

/// How many reports to keep. Enough to see a pattern, few enough that a crash
/// loop cannot fill the disk.
const KEEP_REPORTS: usize = 10;

const NAME_PREFIX: &str = "crash-";
const NAME_SUFFIX: &str = ".log";

/// A stored crash, newest-first being the only ordering anyone asks for.
#[derive(Debug, Clone)]
pub struct CrashReport {
    /// Unix seconds, parsed back out of the file name.
    pub ts: u64,
    pub content: String,
}

/// `~/.colima-ui/crashes`.
fn crash_dir() -> std::path::PathBuf {
    crate::path_util::app_data_dir().join("crashes")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Distinguishes reports written within the same second.
///
/// A crash loop is exactly when several reports arrive at once, and it is also
/// exactly when they matter most — without this, a process crashing twice in one
/// second leaves one file, because the second write lands on the first's name.
static SEQUENCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// File name for a report: `crash-<ts>-<seq>.log`.
///
/// The timestamp is zero-padded so it sorts as a number, but nothing depends on
/// that any more — [`parse_ts`] reads the value back and callers order by the
/// parsed `u64`. Padding is kept because it makes a directory listing readable.
fn report_name(ts: u64, seq: u32) -> String {
    format!("{NAME_PREFIX}{ts:010}-{seq:04}{NAME_SUFFIX}")
}

/// The timestamp out of a report file name, or `None` if this is not one.
///
/// Used as the filter *and* the sort key, so a stray `crash-zzz.log` is not a
/// report at all rather than a report that sorts first and shadows the real one.
fn parse_ts(name: &str) -> Option<u64> {
    let rest = name.strip_prefix(NAME_PREFIX)?.strip_suffix(NAME_SUFFIX)?;
    // `crash-<ts>-<seq>` — take the timestamp; tolerate the older name shape
    // without a sequence so reports written before this change still parse.
    rest.split('-').next()?.parse().ok()
}

/// Report file names in `dir`, each with its parsed timestamp, newest last.
///
/// Only regular files count. A *directory* called `crash-0.log` would otherwise
/// be counted toward the retention limit and never deleted, leaving the
/// directory permanently one over its cap.
fn sorted_reports(dir: &std::path::Path) -> Vec<(u64, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut reports: Vec<(u64, String)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            Some((parse_ts(&name)?, name))
        })
        .collect();

    reports.sort_unstable();
    reports
}

/// The directory is a parameter so tests never touch the real `~/.colima-ui`.
///
/// Every failure is swallowed. This runs inside a panic hook, where the process
/// is already unwinding and there is nobody left to report a second failure to —
/// an `unwrap` here would turn a crash we could have recorded into an abort.
fn write_report_in(dir: &std::path::Path, ts: u64, report: &str) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let seq = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(report_name(ts, seq));
    if std::fs::write(&path, report).is_err() {
        return;
    }

    // A crash report is redacted, not sanitised — treat it as the user's own.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    rotate_in(dir, KEEP_REPORTS);
}

/// Keep the newest `keep` reports, delete the rest.
fn rotate_in(dir: &std::path::Path, keep: usize) {
    let reports = sorted_reports(dir);
    if reports.len() <= keep {
        return;
    }
    for (_, name) in &reports[..reports.len() - keep] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

fn latest_report_in(dir: &std::path::Path) -> Option<CrashReport> {
    // Walk newest-first and take the first that is actually readable. A single
    // unreadable file must not hide the reports behind it — returning `None`
    // there would report "no crash" to a user who had one.
    sorted_reports(dir)
        .into_iter()
        .rev()
        .find_map(|(ts, name)| {
            std::fs::read_to_string(dir.join(&name))
                .ok()
                .map(|content| CrashReport { ts, content })
        })
}

/// The most recent stored crash, if there is one.
///
/// Read by the diagnostic bundle, so the user can see what the app was doing
/// when it died without having had a terminal open at the time.
pub fn latest_report() -> Option<CrashReport> {
    latest_report_in(&crash_dir())
}

/// Install the redacting panic hook. Call once, early in startup.
///
/// The report is redacted, printed to stderr, and written to `~/.colima-ui/
/// crashes`. Transmission is still deferred — nothing is sent anywhere; the
/// diagnostic bundle picks the file up only when the user asks it to.
///
/// It deliberately does not swallow the panic: behaviour is unchanged except
/// that the message is redacted and kept.
pub fn install() {
    panic::set_hook(Box::new(|info| {
        // A panic raised *inside* the panic hook is a double panic, which aborts
        // immediately — no unwinding, no report, exit 134. Catching here means
        // the worst case is a crash we failed to record, rather than a crash
        // that turns into an abort because recording it went wrong.
        let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let payload = payload_to_string(info.payload());
            let location = info.location().map(|l| format!("{}:{}", l.file(), l.line()));
            handle_panic(&crash_dir(), unix_now(), location.as_deref(), &payload);
        }));
    }));
}

/// Everything the hook does, with the directory and clock passed in.
///
/// Split out so the composition — redact, print, prepend the version header,
/// persist — is testable. Closing over `crash_dir()` inside the hook would have
/// left that assembly verifiable only by crashing the real app.
fn handle_panic(dir: &std::path::Path, ts: u64, location: Option<&str>, payload: &str) {
    let report = redacted_report(location, &truncate_payload(payload));

    // Version and platform are the first two questions anyone asks of a crash
    // report, and cost one `format!`. Nothing richer is collected — the process
    // is unwinding, and gathering context means running more code in a state we
    // already know is bad.
    let stored = format!(
        "colima-ui {} on {} {}\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        report
    );

    // Persist BEFORE writing to stderr. `eprintln!` panics if the write fails —
    // a closed pipe is enough, e.g. `colima-ui 2>&1 | head` — and a panic here
    // is a double panic that aborts the process. Doing the durable thing first
    // means a broken stderr costs the message, not the report.
    write_report_in(dir, ts, &stored);

    // `writeln!` returns a Result instead of panicking, so a dead stderr is
    // simply ignored.
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{report}");
}

/// Cap the panic payload before anything else touches it.
///
/// `panic!("{:?}", huge)` can produce an arbitrarily large string, and it would
/// otherwise be cloned through `redacted_report`, run through thirteen regexes,
/// cloned again into the header, and written whole — ten times over, once per
/// retained report. The first 64 KiB carries the message and the useful part of
/// any payload; the rest is noise nobody reads.
fn truncate_payload(payload: &str) -> std::borrow::Cow<'_, str> {
    const LIMIT: usize = 64 * 1024;
    if payload.len() <= LIMIT {
        return std::borrow::Cow::Borrowed(payload);
    }
    // Cut on a char boundary so the result stays valid UTF-8.
    let mut end = LIMIT;
    while end > 0 && !payload.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!(
        "{}\n… truncated, {} bytes total",
        &payload[..end],
        payload.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that removes itself. Tests must never write into the
    /// real `~/.colima-ui/crashes` — that is the user's data, and a test that
    /// rotates it would delete evidence of a real crash.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("colimaui-crash-test-{tag}"));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("temp dir is creatable");
            Self(p)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_written_report_can_be_read_back_with_its_timestamp() {
        let dir = TempDir::new("round-trip");
        write_report_in(dir.path(), 1_700_000_000, "panic at foo.rs:1: boom");

        let latest = latest_report_in(dir.path()).expect("report was written");
        assert_eq!(latest.ts, 1_700_000_000);
        assert!(latest.content.contains("boom"));
    }

    #[test]
    fn the_newest_report_wins_regardless_of_write_order() {
        let dir = TempDir::new("newest");
        write_report_in(dir.path(), 1_700_000_050, "second");
        write_report_in(dir.path(), 1_700_000_010, "first");

        // Name-sorting is time-sorting only because the timestamp is zero-padded.
        let latest = latest_report_in(dir.path()).expect("reports were written");
        assert_eq!(latest.ts, 1_700_000_050);
        assert!(latest.content.contains("second"));
    }

    #[test]
    fn rotation_keeps_the_newest_ten() {
        let dir = TempDir::new("rotate");
        for i in 0..12 {
            write_report_in(dir.path(), 1_700_000_000 + i, &format!("crash {i}"));
        }

        let kept = sorted_reports(dir.path());
        assert_eq!(kept.len(), KEEP_REPORTS, "kept: {kept:?}");
        assert_eq!(kept.last().expect("non-empty").0, 1_700_000_011, "newest was dropped");
        assert_eq!(kept.first().expect("non-empty").0, 1_700_000_002, "oldest was kept");
    }

    /// Two crashes in the same second must both survive. A crash loop is when
    /// reports matter most, and it is exactly when they collide.
    #[test]
    fn two_reports_in_the_same_second_both_survive() {
        let dir = TempDir::new("same-second");
        write_report_in(dir.path(), 1_700_000_000, "first");
        write_report_in(dir.path(), 1_700_000_000, "second");

        assert_eq!(sorted_reports(dir.path()).len(), 2);
    }

    /// A junk file that sorts after the real reports must not shadow them.
    /// Lexicographic `.max()` over unvalidated names returned this one and
    /// reported `ts: 0`.
    #[test]
    fn a_malformed_name_is_not_mistaken_for_the_newest_report() {
        let dir = TempDir::new("junk-name");
        write_report_in(dir.path(), 1_700_000_000, "the real crash");
        std::fs::write(dir.path().join("crash-zzz.log"), "junk").expect("writable");

        let latest = latest_report_in(dir.path()).expect("the real report is found");
        assert_eq!(latest.ts, 1_700_000_000);
        assert!(latest.content.contains("the real crash"));
    }

    /// A directory named like a report was counted toward the cap and could
    /// never be deleted, so the folder settled one over its limit forever.
    #[test]
    fn a_directory_named_like_a_report_does_not_block_rotation() {
        let dir = TempDir::new("dir-decoy");
        std::fs::create_dir(dir.path().join(report_name(1, 0))).expect("dir is creatable");
        for i in 0..12 {
            write_report_in(dir.path(), 1_700_000_000 + i, &format!("crash {i}"));
        }

        assert_eq!(sorted_reports(dir.path()).len(), KEEP_REPORTS);
    }

    /// An unreadable newest file must not hide the readable one behind it.
    #[test]
    fn an_unreadable_newest_report_falls_back_to_the_one_before() {
        let dir = TempDir::new("unreadable-newest");
        write_report_in(dir.path(), 1_700_000_000, "older but readable");
        // A path that parses as a report but cannot be read as a string.
        std::fs::write(dir.path().join(report_name(1_700_000_999, 0)), [0xff, 0xfe])
            .expect("writable");

        let latest = latest_report_in(dir.path()).expect("falls back");
        assert!(latest.content.contains("older but readable"));
    }

    /// A huge payload must not be stored, redacted, or cloned in full.
    #[test]
    fn an_enormous_payload_is_truncated_before_it_is_stored() {
        let dir = TempDir::new("huge");
        let payload = "x".repeat(400_000);
        handle_panic(dir.path(), 1_700_000_000, None, &payload);

        let stored = latest_report_in(dir.path()).expect("written").content;
        assert!(stored.len() < 100_000, "stored {} bytes", stored.len());
        assert!(stored.contains("truncated"), "no truncation marker: {}", &stored[..80]);
    }

    #[test]
    fn truncation_cuts_on_a_character_boundary() {
        // A multi-byte char straddling the limit must not be split.
        let payload = "é".repeat(64 * 1024);
        let out = truncate_payload(&payload);
        assert!(out.contains("truncated"));
        // Round-tripping through &str already proves validity; assert it is not
        // the untouched input.
        assert!(out.len() < payload.len());
    }

    /// The panic hook must survive a directory it cannot write to. Losing the
    /// report is acceptable; aborting the process while it is already unwinding
    /// is not.
    #[test]
    fn an_unwritable_directory_is_survived_silently() {
        let dir = TempDir::new("unwritable");
        let target = dir.path().join("nested");
        std::fs::write(&target, "I am a file, not a directory").expect("file is writable");

        // `create_dir_all` fails because `target` exists and is not a directory.
        write_report_in(&target, 1_700_000_000, "boom");
        assert!(latest_report_in(&target).is_none());
    }

    #[test]
    fn an_empty_directory_reports_no_crash() {
        let dir = TempDir::new("empty");
        assert!(latest_report_in(dir.path()).is_none());
    }

    /// Unrelated files in the directory are not mistaken for reports.
    #[test]
    fn only_crash_files_are_considered() {
        let dir = TempDir::new("mixed");
        std::fs::write(dir.path().join("notes.txt"), "hello").expect("writable");
        assert!(latest_report_in(dir.path()).is_none());

        write_report_in(dir.path(), 1_700_000_000, "real crash");
        let latest = latest_report_in(dir.path()).expect("real report found");
        assert!(latest.content.contains("real crash"));
    }

    /// The whole hook body: a panic in, a redacted file with a version header
    /// out. This is what `install()` runs; the only thing it does not cover is
    /// `panic::set_hook` itself.
    #[test]
    fn a_panic_produces_a_readable_report_with_version_context() {
        let dir = TempDir::new("handle-panic");
        handle_panic(
            dir.path(),
            1_700_000_000,
            Some("src/commands/containers.rs:42"),
            "called `Option::unwrap()` on a `None` value",
        );

        let stored = latest_report_in(dir.path()).expect("a report was written").content;
        assert!(stored.contains(env!("CARGO_PKG_VERSION")), "version missing: {stored}");
        assert!(stored.contains(std::env::consts::OS), "os missing: {stored}");
        assert!(stored.contains("containers.rs:42"), "location missing: {stored}");
        assert!(stored.contains("Option::unwrap()"), "message missing: {stored}");
    }

    /// A panic message carrying a credential must not reach the file.
    #[test]
    fn a_panic_carrying_a_secret_is_redacted_before_it_is_stored() {
        let dir = TempDir::new("handle-panic-secret");
        handle_panic(
            dir.path(),
            1_700_000_000,
            None,
            "POSTGRES_PASSWORD=hunter2 rejected by server",
        );

        let stored = latest_report_in(dir.path()).expect("a report was written").content;
        assert!(!stored.contains("hunter2"), "password leaked to disk: {stored}");
    }

    /// The stored file must already be redacted — there is no second pass.
    #[test]
    fn what_is_written_is_what_was_redacted() {
        let dir = TempDir::new("redacted");
        let report = redacted_report(
            Some("src/commands/ai_chat.rs:180"),
            "error sending request for url (https://x.test/v1/models?key=AIzaSyC1234567890abcdefghijklmnop)",
        );
        write_report_in(dir.path(), 1_700_000_000, &report);

        let stored = latest_report_in(dir.path()).expect("report was written").content;
        assert!(!stored.contains("AIzaSyC1234567890abcdefghijklmnop"), "key leaked to disk: {stored}");
        assert!(stored.contains("ai_chat.rs:180"), "location should survive: {stored}");
    }

    #[test]
    fn report_redacts_api_key_in_panic_message() {
        // The exact leak this exists to stop: a panic carrying a provider key in
        // a request URL.
        let msg = "error sending request for url \
                   (https://generativelanguage.googleapis.com/v1beta/models?key=AIzaSyC1234567890abcdefghijklmnop)";
        let report = redacted_report(Some("src/commands/ai_chat.rs:180"), msg);
        assert!(!report.contains("AIzaSyC1234567890abcdefghijklmnop"), "key leaked: {report}");
        assert!(report.contains("redacted"));
        // Location is kept — it is the useful, non-sensitive part.
        assert!(report.contains("ai_chat.rs:180"));
    }

    #[test]
    fn report_redacts_bearer_token() {
        let msg = "auth failed: Authorization: Bearer sk-ant-abcdefghijklmnopqrstuvwxyz012345";
        let report = redacted_report(None, msg);
        assert!(!report.contains("sk-ant-abcdefghijklmnopqrstuvwxyz012345"), "token leaked: {report}");
    }

    #[test]
    fn report_keeps_a_harmless_message_readable() {
        let report = redacted_report(Some("src/foo.rs:12"), "index out of bounds: the len is 3 but the index is 5");
        assert!(report.contains("index out of bounds"));
        assert!(report.contains("foo.rs:12"));
    }
}
