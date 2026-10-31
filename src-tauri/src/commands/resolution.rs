//! Environment resolution record — "what does the app believe, and why?"
//!
//! Backend for the Environment page: a typed snapshot of every path/env
//! decision this process made at startup, with the rule that made it and an
//! availability check, plus the launchd-vs-user-shell environment duality
//! (a GUI app's process env is whatever launchd gave it; the user's shell
//! env is a different animal — showing both is how "works in terminal but
//! not from the Dock" becomes a visible row instead of a debugging week).
//!
//! Resolution mirrors colima 0.10.x itself (see instance_reader::colima_home
//! and path_util::detect_docker_host — both patched to match colima's own
//! config/files.go order).

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use crate::instance_reader;
use crate::path_util;
use crate::shell_env;

/// One decision the app made. `rule` is the provenance tier:
/// "env" (inherited), "auto-resolve" (filesystem heuristics), "fallback".
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentRow {
    pub key: String,
    pub rule: String,
    pub chosen: String,
    pub detail: Option<String>,
    pub health: Option<String>,
}

/// One env var, seen through both lenses that matter.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarDuality {
    pub key: String,
    /// What this process (launchd-spawned) actually received.
    pub runtime: Option<String>,
    /// What $SHELL -c (non-login) exports — the shell-side of the comparison.
    pub shell: Option<String>,
    pub diverges: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub rows: Vec<EnvironmentRow>,
    pub env_duality: Vec<EnvVarDuality>,
    /// Non-fatal reason the shell snapshot is incomplete, if it is.
    pub shell_snapshot_error: Option<String>,
}

/// Env vars worth tracking in the duality view. PATH is the one that bites
/// (launchd PATH is /usr/bin:/bin — every CLI the app shells out to lives in
/// /opt/homebrew/bin); the others are the engine-relocation knobs.
const TRACKED_ENV_KEYS: &[&str] = &[
    "COLIMA_HOME",
    "LIMA_HOME",
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

/// Pull a flat summary out of a colima.yaml for display: the keys that
/// describe the profile at a glance. Pure and testable.
fn parse_colima_yaml_summary(text: &str) -> String {
    let wanted = ["cpu", "memory", "disk", "runtime", "vmType", "mountType"];
    let mut parts: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
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

fn kubeconfig_row(rows: &mut Vec<EnvironmentRow>) {
    // colima's kubernetes context honors KUBECONFIG; else the standard path.
    let path = std::env::var("KUBECONFIG").map_or_else(
        |_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".kube")
                .join("config")
        },
        PathBuf::from,
    );
    let (rule, detail, health) = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let contexts = text.matches("- context:").count();
            let server = text
                .lines()
                .find_map(|l| l.trim().strip_prefix("server: "))
                .unwrap_or("(no server found)");
            (
                if std::env::var("KUBECONFIG").is_ok() {
                    "env".to_string()
                } else {
                    "fallback".to_string()
                },
                format!("{} context(s), server: {}", contexts, server),
                Some("readable".to_string()),
            )
        }
        Err(e) => (
            "fallback".to_string(),
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

    // colima home — through the patched resolver (env/legacy/XDG order).
    rows.push(file_exists_row(
        "colima-home",
        "auto-resolve",
        &instance_reader::colima_home(),
        "resolution mirrors colima 0.10.x: COLIMA_HOME(dir exists) > ~/.colima > ~/.config/colima > ~/.colima",
    ));

    // lima home — same base, or the LIMA_HOME override (honored unconditionally).
    let lima = std::env::var("LIMA_HOME").map_or_else(
        |_| instance_reader::colima_home().join("_lima"),
        PathBuf::from,
    );
    rows.push(file_exists_row(
        "lima-home",
        if std::env::var("LIMA_HOME").is_ok() {
            "env"
        } else {
            "auto-resolve"
        },
        &lima,
        "VM instances + data disks live here",
    ));

    // docker socket — discovery with a live probe (this is the row that
    // answers "why is my container list empty").
    rows.push(match path_util::detect_docker_host() {
        Some((host, ctx)) => EnvironmentRow {
            key: "docker-socket".to_string(),
            rule: if std::env::var("DOCKER_HOST").is_ok() {
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

fn build_env_duality(
    snapshot: Option<&BTreeMap<String, String>>,
) -> (Vec<EnvVarDuality>, Option<String>) {
    let mut out = Vec::new();
    let mut err = None;
    for key in TRACKED_ENV_KEYS {
        let runtime = std::env::var(key).ok();
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
    (out, err.take())
}

/// #[tauri::command] GET /api/environment/resolutions
#[tauri::command]
pub fn get_environment_resolutions() -> EnvironmentReport {
    // the shell snapshot runs at most ONCE per process (success or failure are
    // both cached) — re-scan never re-spawns it
    let (duality, snapshot_error) = match shell_env::snapshot() {
        Ok(snapshot) => build_env_duality(Some(&snapshot)),
        Err(e) => (build_env_duality(None).0, Some(e)),
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
        let text = "cpu: 4\nmemory: 8GiB\ndisk: 60GiB\nruntime: docker\nvmType: \"vz\"\n# comment\ncpus: 99\nkubernetes:\n  enabled: true\n";
        let summary = parse_colima_yaml_summary(text);
        assert!(summary.contains("cpu: 4"), "got: {}", summary);
        assert!(summary.contains("runtime: docker"), "got: {}", summary);
        assert!(summary.contains("vmType: \"vz\""), "got: {}", summary);
        // must not pick up commented or nested keys
        assert!(!summary.contains("99"), "got: {}", summary);
        assert!(!summary.contains("enabled"), "got: {}", summary);
    }

    #[test]
    fn test_parse_colima_yaml_summary_empty() {
        assert_eq!(parse_colima_yaml_summary(""), "");
        assert_eq!(parse_colima_yaml_summary("only comments\n# cpu: 4\n"), "");
    }

    #[test]
    fn test_tracked_keys_are_declared() {
        // the duality view tracks the relocation knobs + PATH; if a new knob
        // is added to env.zsh, add it here too
        let expected = [
            "COLIMA_HOME",
            "LIMA_HOME",
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

}
