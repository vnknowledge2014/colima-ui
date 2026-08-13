//! One source of truth for "what is installed and usable on this machine".
//!
//! The pieces existed already — `check_system`, `check_tool`, `get_platform` —
//! but each returned a different shape and the frontend stitched them together
//! in `SetupWizard.svelte`. Every other page that needed to say "Colima isn't
//! installed" had to repeat that work, so most of them showed an empty table
//! instead.
//!
//! This module answers one question per tool: *can the user use it right now?*
//!
//! Deliberately NOT built on `helpers::SYSTEM_INFO_CACHE`: that cache has a
//! hardcoded 300s TTL, is shared with `/api/system` and `/api/version`, and has
//! no way to invalidate. Capability state changes the moment an instance
//! starts, so it needs its own short-lived cache that can be dropped on demand.

use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::commands::system::get_version;

/// How long a detection result is reused. Short, because the answer changes
/// whenever the user installs something or starts an instance; the poller also
/// invalidates explicitly on instance transitions.
const CACHE_TTL: Duration = Duration::from_secs(15);

/// Can the user use this tool right now?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    /// The binary is not on PATH.
    Missing,
    /// Installed, but the daemon or VM it needs is not up.
    InstalledNotRunning,
    /// Usable now.
    Running,
    /// Detection failed (timed out, permission error).
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Stable identifier the frontend switches on.
    pub id: String,
    /// Display name, not translated — these are product names.
    pub name: String,
    pub state: CapabilityState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Shell command that installs it, for the platform we are running on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
    /// Knowledge Base article slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
}

struct CapCache {
    data: Vec<Capability>,
    fetched_at: Instant,
}

static CACHE: LazyLock<Mutex<Option<CapCache>>> = LazyLock::new(|| Mutex::new(None));

/// Drop the cached result so the next read re-detects.
///
/// Called when an instance starts or stops: that flips Colima and Docker
/// between `InstalledNotRunning` and `Running`, and waiting out the TTL would
/// leave the UI telling the user to start something they just started.
pub fn invalidate() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

/// Read the cache if it is still fresh.
///
/// Takes and releases the lock inside this function on purpose — the lock must
/// never be held across the detection work, which spawns processes.
fn cached() -> Option<Vec<Capability>> {
    let guard = CACHE.lock().ok()?;
    let entry = guard.as_ref()?;
    if entry.fetched_at.elapsed() < CACHE_TTL {
        Some(entry.data.clone())
    } else {
        None
    }
}

fn store(data: &[Capability]) {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CapCache {
            data: data.to_vec(),
            fetched_at: Instant::now(),
        });
    }
}

/// Install command for a tool on the current platform.
fn install_hint(id: &str) -> Option<String> {
    let macos = cfg!(target_os = "macos");
    let hint = match (id, macos) {
        ("colima", true) => "brew install colima",
        ("colima", false) => "See the Colima install guide for your distribution",
        ("docker", true) => "brew install docker",
        ("docker", false) => "Install the docker CLI from your package manager",
        ("docker-compose", true) => "brew install docker-compose",
        ("docker-compose", false) => "Install the docker compose plugin",
        ("lima", true) => "brew install lima",
        ("lima", false) => "See the Lima install guide for your distribution",
        ("kubectl", true) => "brew install kubectl",
        ("kubectl", false) => "Install kubectl from your package manager",
        ("trivy", true) => "brew install trivy",
        ("trivy", false) => "See the Trivy install guide for your distribution",
        _ => return None,
    };
    Some(hint.to_string())
}

fn doc_id(id: &str) -> Option<String> {
    let slug = match id {
        "colima" => "install-colima",
        "docker" | "docker-compose" => "install-docker-cli",
        "kubectl" => "install-kubectl",
        "lima" => "install-colima",
        "trivy" => "install-trivy",
        "falco" => "install-falco",
        _ => return None,
    };
    Some(slug.to_string())
}

fn capability(id: &str, name: &str, state: CapabilityState, version: Option<String>) -> Capability {
    Capability {
        id: id.to_string(),
        name: name.to_string(),
        state,
        version: version.map(|v| v.lines().next().unwrap_or("").trim().to_string()),
        install_hint: matches!(state, CapabilityState::Missing).then(|| install_hint(id)).flatten(),
        doc_id: doc_id(id),
    }
}

/// Detect everything. Blocking — callers must run it off the async runtime.
fn detect_all_blocking() -> Vec<Capability> {
    // Colima: installed if the binary answers, running if any instance is up.
    let colima_version = get_version("colima", &["version"]);
    let colima_state = match &colima_version {
        None => CapabilityState::Missing,
        Some(_) => {
            let any_running = crate::instance_reader::list_instances_fast()
                .iter()
                .any(|i| i.status.eq_ignore_ascii_case("running"));
            if any_running {
                CapabilityState::Running
            } else {
                CapabilityState::InstalledNotRunning
            }
        }
    };

    // Docker: the CLI can be present while the daemon is unreachable, which is
    // the single most common confusing state for this app's users.
    let docker_version = get_version("docker", &["--version"]);
    let docker_state = match &docker_version {
        None => CapabilityState::Missing,
        Some(_) => {
            if get_version("docker", &["info", "--format", "{{.ServerVersion}}"]).is_some() {
                CapabilityState::Running
            } else {
                CapabilityState::InstalledNotRunning
            }
        }
    };

    // Pure CLIs: there is no daemon, so "installed" means "usable".
    let compose_version = get_version("docker", &["compose", "version"]);
    let lima_version = get_version("limactl", &["--version"]);
    let kubectl_version = get_version("kubectl", &["version", "--client"]);
    // The vulnerability scanner. Absent on most machines and deliberately not
    // bundled: its database alone is 1.2 GB.
    let trivy_version = get_version("trivy", &["--version"]);

    let plain = |v: &Option<String>| {
        if v.is_some() {
            CapabilityState::Running
        } else {
            CapabilityState::Missing
        }
    };

    vec![
        capability("colima", "Colima", colima_state, colima_version),
        capability("docker", "Docker CLI", docker_state, docker_version),
        capability(
            "docker-compose",
            "Docker Compose",
            plain(&compose_version),
            compose_version,
        ),
        capability("lima", "Lima", plain(&lima_version), lima_version),
        capability("kubectl", "kubectl", plain(&kubectl_version), kubectl_version),
        capability("trivy", "Trivy", plain(&trivy_version), trivy_version),
    ]
}

/// Capability list for the whole app, cached.
#[tauri::command]
pub async fn get_system_capabilities() -> Result<Vec<Capability>, crate::error::ColimaError> {
    if let Some(hit) = cached() {
        return Ok(hit);
    }

    // Detection spawns several processes; keep it off the async runtime and,
    // critically, hold no lock while it runs.
    let detected = tokio::task::spawn_blocking(detect_all_blocking)
        .await
        .map_err(|e| crate::error::ColimaError::internal(format!("Detection task failed: {}", e)))?;

    store(&detected);
    Ok(detected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tools_carry_an_install_hint_but_present_ones_do_not() {
        let missing = capability("colima", "Colima", CapabilityState::Missing, None);
        assert!(missing.install_hint.is_some());

        let running = capability(
            "colima",
            "Colima",
            CapabilityState::Running,
            Some("colima version 0.6.0".into()),
        );
        assert!(
            running.install_hint.is_none(),
            "telling a user to install something they have is noise"
        );
    }

    #[test]
    fn version_is_reduced_to_its_first_line() {
        let c = capability(
            "docker",
            "Docker CLI",
            CapabilityState::Running,
            Some("Docker version 27.0.3, build 1a2b3c\nextra line\n".into()),
        );
        assert_eq!(c.version.as_deref(), Some("Docker version 27.0.3, build 1a2b3c"));
    }

    #[test]
    fn unknown_tools_have_neither_hint_nor_doc() {
        let c = capability("nonesuch", "Nonesuch", CapabilityState::Missing, None);
        assert!(c.install_hint.is_none());
        assert!(c.doc_id.is_none());
    }

    #[test]
    fn state_serializes_as_snake_case_for_the_frontend_contract() {
        let json = serde_json::to_string(&CapabilityState::InstalledNotRunning).expect("serialize");
        assert_eq!(json, "\"installed_not_running\"");
    }

    #[test]
    fn invalidate_clears_a_stored_result() {
        store(&[capability("colima", "Colima", CapabilityState::Running, None)]);
        assert!(cached().is_some());
        invalidate();
        assert!(cached().is_none(), "invalidate must force a re-detect");
    }
}
