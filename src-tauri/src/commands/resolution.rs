//! Environment resolution record — "what does the app believe, and why?"
//!
//! Backend for the Environment page: a typed snapshot of every path/env
//! decision this process made at startup, with the rule that made it and an
//! availability check, plus the launchd-vs-user-shell environment duality
//! (a GUI app's process env is whatever launchd gave it; the user's shell
//! env is a different animal — showing both is how "works in terminal but
//! not from the Dock" becomes a visible row instead of a debugging week).
//!
//! Resolution mirrors colima 0.10.x itself (see `instance_reader::colima_home`
//! and `path_util::detect_docker_host` — both aligned with colima's own
//! `config/files.go` order).

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::instance_reader;
use crate::path_util;
use crate::shell_env;

/// One decision the app made. `rule` is the provenance tier:
/// "env" (inherited), "auto-resolve" (filesystem heuristics), "discovered", or "fallback".
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentRow {
    pub key: String,
    pub rule: String,
    pub chosen: String,
    pub detail: Option<String>,
    pub health: Option<String>,
}

/// One env var, seen through both lenses that matter.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarDuality {
    pub key: String,
    /// What this process (launchd-spawned) originally received before `shell_env::adopt()`.
    pub runtime: Option<String>,
    /// What `$SHELL -c` (non-login) exports — the shell side of the comparison.
    pub shell: Option<String>,
    pub diverges: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub rows: Vec<EnvironmentRow>,
    pub env_duality: Vec<EnvVarDuality>,
    /// Non-fatal reason the shell snapshot is incomplete, if it is.
    pub shell_snapshot_error: Option<String>,
}

/// Env vars worth tracking in the duality view. `PATH` is the one that bites
/// (launchd `PATH` is `/usr/bin:/bin` — every CLI the app shells out to lives in
/// `/opt/homebrew/bin`); the others are the engine/config relocation knobs.
const TRACKED_ENV_KEYS: &[&str] = &[
    "COLIMA_HOME",
    "LIMA_HOME",
    "XDG_CONFIG_HOME",
    "DOCKER_CONFIG",
    "DOCKER_HOST",
    "KUBECONFIG",
    "PATH",
];

// ===== row builders =====

fn file_exists_row(key: &str, rule: &str, path: &Path, detail_extra: &str) -> EnvironmentRow {
    EnvironmentRow {
        key: key.to_string(),
        rule: rule.to_string(),
        chosen: path.display().to_string(),
        detail: Some(detail_extra.to_string()),
        health: Some(if path.exists() {
            "exists".to_string()
        } else {
            "missing".to_string()
        }),
    }
}

/// Pull a flat summary out of a `colima.yaml` for display: the keys that
/// describe the profile at a glance. Pure and testable.
fn parse_colima_yaml_summary(text: &str) -> String {
    let wanted = ["cpu", "memory", "disk", "runtime", "vmType", "mountType"];
    let mut parts: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed_raw = line.split('#').next().unwrap_or("");
        // Top-level keys only (nested blocks like `kubernetes:\n  enabled: true` carry leading spaces).
        if trimmed_raw.starts_with(' ') || trimmed_raw.starts_with('\t') {
            continue;
        }
        let line = trimmed_raw.trim();
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim();
        if wanted.contains(&k) && !v.trim().is_empty() {
            parts.push(format!("{}: {}", k, v.trim()));
        }
    }
    parts.join(" · ")
}

fn colima_profile_summary_row(rows: &mut Vec<EnvironmentRow>) {
    let path = instance_reader::colima_home()
        .join("default")
        .join("colima.yaml");
    let (rule, chosen, detail, health) = match std::fs::read_to_string(&path) {
        Ok(text) => (
            "auto-resolve".to_string(),
            path.display().to_string(),
            Some(parse_colima_yaml_summary(&text)),
            Some("readable".to_string()),
        ),
        Err(e) => (
            "auto-resolve".to_string(),
            path.display().to_string(),
            Some(format!("not readable: {}", e)),
            Some("missing".to_string()),
        ),
    };
    rows.push(EnvironmentRow {
        key: "colima-profile-config".to_string(),
        rule,
        chosen,
        detail,
        health,
    });
}

/// Resolve primary kubeconfig file path from `KUBECONFIG` (supporting multi-path
/// `a:b` lists via `std::env::split_paths`) or default to `$HOME/.kube/config`.
fn resolve_primary_kubeconfig(kubeconfig_env: Option<&str>, home: &str) -> (PathBuf, &'static str) {
    if let Some(raw) = kubeconfig_env.map(str::trim).filter(|v| !v.is_empty()) {
        if let Some(first) = std::env::split_paths(raw).find(|p| !p.as_os_str().is_empty()) {
            return (first, "env");
        }
    }
    (
        PathBuf::from(home).join(".kube").join("config"),
        "fallback",
    )
}

fn kubeconfig_row(rows: &mut Vec<EnvironmentRow>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let kube_env = std::env::var("KUBECONFIG").ok();
    let (path, rule) = resolve_primary_kubeconfig(kube_env.as_deref(), &home);

    let (detail, health) = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let contexts = text.matches("- context:").count();
            let server = text
                .lines()
                .find_map(|l| l.trim().strip_prefix("server: "))
                .unwrap_or("(no server found)");
            (
                format!("{} context(s), server: {}", contexts, server),
                Some("readable".to_string()),
            )
        }
        Err(e) => (
            format!("not readable: {}", e),
            Some("missing".to_string()),
        ),
    };
    rows.push(EnvironmentRow {
        key: "kubeconfig".to_string(),
        rule: rule.to_string(),
        chosen: path.display().to_string(),
        detail: Some(detail),
        health,
    });
}

fn build_rows() -> Vec<EnvironmentRow> {
    let mut rows = Vec::new();

    let colima_env_valid = std::env::var("COLIMA_HOME")
        .ok()
        .is_some_and(|d| !d.trim().is_empty() && Path::new(d.trim()).is_dir());
    rows.push(file_exists_row(
        "colima-home",
        if colima_env_valid {
            "env"
        } else {
            "auto-resolve"
        },
        &instance_reader::colima_home(),
        "resolution mirrors colima 0.10.x: COLIMA_HOME(dir exists) > ~/.colima(state) > ~/.config/colima > ~/.colima",
    ));

    let lima = path_util::lima_home();
    let lima_from_env = std::env::var("LIMA_HOME").is_ok_and(|v| !v.trim().is_empty());
    rows.push(file_exists_row(
        "lima-home",
        if lima_from_env { "env" } else { "auto-resolve" },
        &lima,
        "VM instances + data disks live here",
    ));

    rows.push(match path_util::detect_docker_host() {
        Some((host, ctx)) => EnvironmentRow {
            key: "docker-socket".to_string(),
            rule: if std::env::var("DOCKER_HOST").is_ok_and(|v| !v.trim().is_empty()) {
                "env".to_string()
            } else {
                "discovered".to_string()
            },
            chosen: host,
            detail: Some(format!("active context: {}", ctx)),
            health: Some("live".to_string()),
        },
        None => EnvironmentRow {
            key: "docker-socket".to_string(),
            rule: "auto-resolve".to_string(),
            chosen: "not found".to_string(),
            detail: Some(
                "probe_docker_socket found no live socket; is colima running?".to_string(),
            ),
            health: Some("unreachable".to_string()),
        },
    });

    kubeconfig_row(&mut rows);
    colima_profile_summary_row(&mut rows);
    rows
}

fn build_env_duality_with<F>(
    snapshot: Option<&BTreeMap<String, String>>,
    mut runtime_lookup: F,
) -> Vec<EnvVarDuality>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut out = Vec::with_capacity(TRACKED_ENV_KEYS.len());
    for key in TRACKED_ENV_KEYS {
        let runtime = runtime_lookup(key);
        let shell = snapshot.and_then(|m| m.get(*key).cloned());
        let diverges = match (&runtime, &shell) {
            (Some(a), Some(b)) => a != b,
            (None, None) => false,
            _ => true,
        };
        out.push(EnvVarDuality {
            key: key.to_string(),
            runtime,
            shell,
            diverges,
        });
    }
    out
}

/// #[tauri::command] GET /api/environment/resolutions
#[tauri::command]
pub fn get_environment_resolutions() -> EnvironmentReport {
    // The shell snapshot runs at most ONCE per process (success or failure are
    // both cached) — re-scan never re-spawns it. `shell_env::launchd_env`
    // returns the pre-adoption process env captured before `shell_env::adopt()`
    // ran at startup, so the duality comparison survives adoption.
    let (duality, snapshot_error) = match shell_env::snapshot() {
        Ok(snapshot) => (
            build_env_duality_with(Some(&snapshot), shell_env::launchd_env),
            None,
        ),
        Err(e) => (
            build_env_duality_with(None, shell_env::launchd_env),
            Some(e),
        ),
    };

    EnvironmentReport {
        rows: build_rows(),
        env_duality: duality,
        shell_snapshot_error: snapshot_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colima_yaml_summary() {
        let text = "cpu: 4\nmemory: 8GiB\ndisk: 60GiB\nruntime: docker\nvmType: \"vz\"\n# comment\ncpus: 99\nkubernetes:\n  enabled: true\n  runtime: containerd\n";
        let summary = parse_colima_yaml_summary(text);
        assert!(summary.contains("cpu: 4"), "got: {}", summary);
        assert!(summary.contains("runtime: docker"), "got: {}", summary);
        assert!(summary.contains("vmType: \"vz\""), "got: {}", summary);
        // Must not pick up commented or indented nested keys.
        assert!(!summary.contains("99"), "got: {}", summary);
        assert!(!summary.contains("enabled"), "got: {}", summary);
        assert!(!summary.contains("containerd"), "got: {}", summary);
    }

    #[test]
    fn test_parse_colima_yaml_summary_empty() {
        assert_eq!(parse_colima_yaml_summary(""), "");
        assert_eq!(parse_colima_yaml_summary("only comments\n# cpu: 4\n"), "");
    }

    #[test]
    fn test_tracked_keys_are_declared() {
        let expected = [
            "COLIMA_HOME",
            "LIMA_HOME",
            "XDG_CONFIG_HOME",
            "DOCKER_CONFIG",
            "DOCKER_HOST",
            "KUBECONFIG",
            "PATH",
        ];
        for k in expected {
            assert!(
                TRACKED_ENV_KEYS.contains(&k),
                "missing tracked env key: {}",
                k
            );
        }
    }

    #[test]
    fn test_build_env_duality_with_preserves_launchd_vs_shell_diff() {
        let mut launchd = BTreeMap::new();
        launchd.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        launchd.insert("KUBECONFIG".to_string(), "/Users/u/.kube/config".to_string());

        let mut shell = BTreeMap::new();
        shell.insert(
            "PATH".to_string(),
            "/opt/homebrew/bin:/usr/bin:/bin".to_string(),
        );
        shell.insert("KUBECONFIG".to_string(), "/Users/u/.kube/config".to_string());
        shell.insert("LIMA_HOME".to_string(), "/Users/u/.lima".to_string());

        let rows = build_env_duality_with(Some(&shell), |k| launchd.get(k).cloned());
        let by_key = |name: &str| rows.iter().find(|r| r.key == name).unwrap();

        let path_row = by_key("PATH");
        assert!(path_row.diverges);
        assert_eq!(path_row.runtime.as_deref(), Some("/usr/bin:/bin"));
        assert_eq!(
            path_row.shell.as_deref(),
            Some("/opt/homebrew/bin:/usr/bin:/bin")
        );

        let kube_row = by_key("KUBECONFIG");
        assert!(!kube_row.diverges);

        let lima_row = by_key("LIMA_HOME");
        assert!(lima_row.diverges);
        assert_eq!(lima_row.runtime, None);
        assert_eq!(lima_row.shell.as_deref(), Some("/Users/u/.lima"));

        let colima_row = by_key("COLIMA_HOME");
        assert!(!colima_row.diverges);
        assert_eq!(colima_row.runtime, None);
        assert_eq!(colima_row.shell, None);
    }

    #[test]
    fn test_resolve_primary_kubeconfig_handles_multi_path_and_fallback() {
        let joined = std::env::join_paths(["/tmp/a.yaml", "/tmp/b.yaml"])
            .unwrap()
            .into_string()
            .unwrap();
        let (first, rule) = resolve_primary_kubeconfig(Some(&joined), "/Users/u");
        assert_eq!(first, PathBuf::from("/tmp/a.yaml"));
        assert_eq!(rule, "env");

        let (fallback, fb_rule) = resolve_primary_kubeconfig(Some("   "), "/Users/u");
        assert_eq!(fallback, PathBuf::from("/Users/u/.kube/config"));
        assert_eq!(fb_rule, "fallback");
    }
}
