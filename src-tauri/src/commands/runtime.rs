use std::process::Command;

/// Detect the active container runtime name and profile.
/// Returns (runtime_name, profile_name, docker_host_socket).
pub fn detect_runtime() -> (String, String, Option<String>) {
    let mut runtime_name = "docker".to_string();
    let mut profile_name = "default".to_string();
    let mut host_sock = None;

    if let Some((host, profile)) = crate::path_util::detect_docker_host() {
        host_sock = Some(host);
        profile_name = profile;
    }

    // Determine runtime from colima status output
    let status_output = Command::new(crate::path_util::resolve_binary("colima"))
        .args(["status", "-p", &profile_name])
        .output();

    if let Ok(output) = status_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("runtime: podman") {
            runtime_name = "podman".to_string();
        } else if stdout.contains("runtime: containerd") {
            runtime_name = "containerd".to_string();
        }
    }

    (runtime_name, profile_name, host_sock)
}

/// Gets the appropriate CLI command (`docker`, `podman`, or `nerdctl`)
/// based on the current active Colima profile's runtime.
pub fn get_runtime_cmd() -> Command {
    let (runtime_name, profile_name, host_sock) = detect_runtime();

    let mut cmd;
    if runtime_name == "containerd" {
        // Colima provides `colima nerdctl` wrapper which correctly executes inside the VM
        cmd = Command::new(crate::path_util::resolve_binary("colima"));
        cmd.args(["-p", &profile_name, "nerdctl"]);
    } else {
        let bin = if runtime_name == "containerd" { "nerdctl" } else { &runtime_name };
        cmd = Command::new(crate::path_util::resolve_binary(bin));
        if let Some(host) = host_sock {
            // For podman and docker, setting DOCKER_HOST works natively on macOS
            cmd.env("DOCKER_HOST", host);
        }
    }

    crate::path_util::apply_path_to_cmd(&mut cmd);
    cmd
}

/// Run a runtime CLI command and collect its output.
///
/// Four modules — containers, volumes, networks and compose — each carried a
/// byte-identical private copy of this, so a change to how the app talks to the
/// runtime had four places to remember. This is that one place.
///
/// **Every CLI call must go through here rather than calling `.output()`
/// directly.** `.output()` blocks the calling thread, which starves the Tokio
/// runtime when several commands are issued at once — and the UI does exactly
/// that, firing containers, images, volumes and networks together on load. The
/// blocking pool absorbs that; the async runtime does not.
///
/// The timeout is the other half: a frozen daemon returns an error instead of
/// hanging forever, which is the difference between a visible failure and a
/// spinner that never stops.
///
/// `timeout` is a parameter rather than a constant so each caller states the
/// wait it expects. Every caller currently passes [`DEFAULT_TIMEOUT`], which is
/// what the four copies used; the parameter exists because that is not right
/// for all of them — `compose up` can pull images for minutes and is cut off at
/// ten seconds today. Fixing that is a deliberate behaviour change and is not
/// part of consolidating the duplicates.
pub async fn run(
    args: Vec<String>,
    timeout: std::time::Duration,
) -> Result<std::process::Output, String> {
    let result = tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(move || {
            get_runtime_cmd()
                .args(args.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
                .output()
                .map_err(|e| format!("Failed to run docker command: {}", e))
        }),
    )
    .await;

    match result {
        Ok(join_result) => join_result.map_err(|e| format!("Task join error: {}", e))?,
        Err(_) => Err("Docker command timed out (daemon may be unresponsive)".to_string()),
    }
}

/// The wait the four merged copies used. Kept as the single default so the
/// consolidation changes no behaviour.
pub const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Returns the active runtime name (e.g., "docker", "podman", "containerd").
#[tauri::command]
pub async fn get_runtime_info() -> Result<String, crate::error::ColimaError> {
    async move {
    let (runtime_name, _, _) = detect_runtime();
    Ok(runtime_name)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frozen daemon must surface as an error, not as a spinner that never
    /// stops. Exercised with a real command that outlives its timeout rather
    /// than by asserting on the constant.
    #[tokio::test]
    async fn a_command_that_outlives_its_timeout_reports_the_timeout() {
        // `version` is harmless and exists on every runtime; the point is the
        // deadline, which is set below anything a process can complete in.
        let err = run(
            vec!["version".to_string()],
            std::time::Duration::from_nanos(1),
        )
        .await
        .expect_err("a one-nanosecond deadline cannot be met");
        assert!(
            err.contains("timed out"),
            "the message must say what happened, got: {err}"
        );
    }

    /// The consolidation must not have changed the wait the four copies used.
    #[test]
    fn the_default_timeout_is_the_one_the_merged_copies_had() {
        assert_eq!(DEFAULT_TIMEOUT, std::time::Duration::from_secs(10));
    }

    /// The merged path still reaches a real runtime.
    ///
    /// Every read in the app now goes through this one function, so "it
    /// compiles" is not evidence it works. Ignored by default because it needs
    /// a running daemon: `cargo test --lib -- --ignored runtime`.
    #[tokio::test]
    #[ignore = "requires a running container runtime"]
    async fn the_merged_path_reaches_a_real_runtime() {
        for args in [vec!["ps".to_string(), "--format".to_string(), "json".to_string()],
                     vec!["volume".to_string(), "ls".to_string()],
                     vec!["compose".to_string(), "ls".to_string()]] {
            let label = args.join(" ");
            let output = run(args, DEFAULT_TIMEOUT)
                .await
                .unwrap_or_else(|e| panic!("`{label}` failed: {e}"));
            assert!(output.status.success(), "`{label}` exited non-zero");
        }
    }
}
