//! The user's shell environment, snapshotted once per app process.
//!
//! A GUI process gets launchd's environment; the user's *shell* env is a
//! different animal (zdot-style setups export DOCKER_CONFIG / LIMA_HOME /
//! DOCKER_HOST at non-interactive shell startup — which is also what the
//! user believes is configured). Consumers:
//! - `commands::resolution` — the duality table
//! - `path_util::lima_home` — where the lima instances actually live, which
//!   plain filesystem heuristics cannot know when launchd's env is bare

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const ENV_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);
static SHELL_SNAPSHOT: OnceLock<Result<BTreeMap<String, String>, String>> = OnceLock::new();

/// Parse raw `KEY=VALUE` lines from `env(1)`.
/// Non-matching lines are dropped, not treated as continuations: multi-line
/// shell values (exported functions) are fundamentally ambiguous in flat
/// env(1) output, and the keys this module tracks are never multi-line.
pub fn parse_env_lines(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let Some(eq) = line.find('=') else {
            continue;
        };
        let key = &line[..eq];
        if key.is_empty() || key.contains(' ') {
            continue;
        }
        map.insert(key.to_string(), line[eq + 1..].to_string());
    }
    map
}

/// `$SHELL -c "command env"` — NON-login (no -l): sources the shell's
/// non-interactive startup only (zsh: .zshenv; bash: BASH_ENV), which is
/// where portable env config lives. -l/-i would pull the user's whole
/// interactive prompt stack into a headless process (omz noise, multi-second
/// cold starts) for env the snapshot does not need.
pub fn snapshot() -> Result<BTreeMap<String, String>, String> {
    SHELL_SNAPSHOT.get_or_init(do_snapshot).clone()
}

/// Non-panicking single-var lookup through the snapshot.
pub fn lookup(key: &str) -> Option<String> {
    snapshot().ok().and_then(|m| m.get(key).cloned())
}

/// The process env as launchd delivered it, captured BEFORE adopt() runs.
/// The duality table renders this as the "This app" column so its
/// launchd-vs-shell diagnosis survives the adoption.
static LAUNCHD_ENV: OnceLock<BTreeMap<String, String>> = OnceLock::new();

/// What the process env had for `key` at first launch (pre-adoption view).
pub fn launchd_env(key: &str) -> Option<String> {
    LAUNCHD_ENV
        .get_or_init(|| TRACKED_KEYS.iter().filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v))).collect())
        .get(key)
        .cloned()
}

const TRACKED_KEYS: &[&str] = &[
    "COLIMA_HOME",
    "LIMA_HOME",
    "DOCKER_CONFIG",
    "DOCKER_HOST",
    "KUBECONFIG",
    "XDG_CONFIG_HOME",
    "PATH",
];

/// Adopt the user's shell env into this process, once, at startup: for each
/// tracked key that is ABSENT here, set it to the value the user's shell has.
/// This is the GUI-app equivalent of "launch through the user's shell" —
/// after this, plain env lookups behave the way a terminal's would. Already
/// set vars are NEVER overwritten (explicit beats clever).
pub fn adopt() {
    // capture the pre-adoption state first (duality table lives on it)
    LAUNCHD_ENV.get_or_init(|| {
        TRACKED_KEYS.iter().filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v))).collect()
    });

    let Ok(shell_env) = snapshot() else {
        return; // snapshot failure already recorded; report path explains it
    };
    for key in TRACKED_KEYS {
        if *key == "PATH" {
            // adopt PATH only when it lacks the package-manager roots (the
            // launchd default): a real shell PATH is a superset in practice
            let ok_path = std::env::var("PATH")
                .is_ok_and(|p| p.contains("/opt/homebrew/bin") || p.contains("/usr/local/bin"));
            if ok_path {
                continue;
            }
        } else if std::env::var(key).is_ok_and(|v| !v.trim().is_empty()) {
            continue; // already set: explicit beats clever
        }
        if let Some(v) = shell_env.get(*key) {
            if !v.trim().is_empty() {
                std::env::set_var(key, v);
            }
        }
    }
}

fn do_snapshot() -> Result<BTreeMap<String, String>, String> {
    let shell = std::env::var("SHELL").map_err(|_| "SHELL not set in process env".to_string())?;
    let mut child = std::process::Command::new(&shell)
        .arg("-c") // NOT -l: non-login, non-interactive. Sources only
        .arg("command env")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("could not spawn {}: {}", shell, e))?;

    let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let mut handle = stdout;
        let _ = handle.read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + ENV_SNAPSHOT_TIMEOUT;
    let output = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("{} -c env exited non-zero: {}", shell, status));
                }
                break reader.join().map_err(|_| "reader panicked".to_string())?;
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                return Err(format!("{} -c env snapshot timed out", shell));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("wait failed: {}", e)),
        }
    };

    Ok(parse_env_lines(&output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_lines() {
        let text = "PATH=/a:/b\nCOLIMA_HOME=/opt/x\nBAD LINE\nNEXT=\nvalue\nHOME=/home/u\n";
        let map = parse_env_lines(text);
        assert_eq!(map.get("PATH").unwrap(), "/a:/b");
        assert_eq!(map.get("COLIMA_HOME").unwrap(), "/opt/x");
        // an empty value is a real entry (NEXT=), and bare continuation lines
        // belong to no key — dropped, not appended
        assert_eq!(map.get("NEXT").unwrap(), "");
        assert_eq!(map.get("HOME").unwrap(), "/home/u");
        assert!(!map.contains_key("BAD"));
    }

    #[test]
    fn test_parse_env_lines_multi_line_value_is_dropped() {
        // multi-line shell values (exported functions) are ambiguous in flat
        // env(1) output — the first KEY= line parses (truncated at the space)
        // and continuation lines are skipped; the tracked keys are never
        // multi-line, so nothing we care about is lost
        let text = "Bash_FUNC=() {\n  echo hi\n}\nOK=1\n";
        let map = parse_env_lines(text);
        assert!(map.contains_key("OK"), "tracked keys must parse normally");
        assert_eq!(map.get("Bash_FUNC").unwrap(), "() {");
        // the continuation lines must not leak into neighbors
        assert_eq!(map.get("OK").unwrap(), "1");
    }
}