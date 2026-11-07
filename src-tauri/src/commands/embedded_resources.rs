//! Compile-time embedded Markdown resources (`system_prompt.md`, `router.md`,
//! `skills/**`, and `kb/**`) with seamless on-disk override in dev/Desktop bundles.
//!
//! Standalone `cui` / `colimaui` binaries can be copied anywhere (e.g. `/usr/local/bin/cui`)
//! where `src-tauri/resources/` is not present on disk. Embedding all `.md` resources via
//! `include_str!` guarantees 100% parity between Desktop IPC, Local HTTP API, MCP, A2A,
//! and standalone CLI execution.

use std::path::{Component, Path, PathBuf};

use crate::commands::system::{build_app_context_from_dir, read_reference_from_dir, PluginsConfig};

/// Static lookup table of all Markdown resources under `src-tauri/resources/`,
/// keyed by normalized relative path (forward slashes, no leading `resources/`).
pub const EMBEDDED_MARKDOWN_RESOURCES: &[(&str, &str)] = &[
    // Core prompt & router
    (
        "system_prompt.md",
        include_str!("../../resources/system_prompt.md"),
    ),
    ("router.md", include_str!("../../resources/router.md")),
    // Skills & references
    (
        "skills/colima_ui/SKILL.md",
        include_str!("../../resources/skills/colima_ui/SKILL.md"),
    ),
    (
        "skills/colima/SKILL.md",
        include_str!("../../resources/skills/colima/SKILL.md"),
    ),
    (
        "skills/colima/references/mount_volumes.md",
        include_str!("../../resources/skills/colima/references/mount_volumes.md"),
    ),
    (
        "skills/colima/references/networking_and_dns.md",
        include_str!("../../resources/skills/colima/references/networking_and_dns.md"),
    ),
    (
        "skills/colima/references/performance_tuning.md",
        include_str!("../../resources/skills/colima/references/performance_tuning.md"),
    ),
    (
        "skills/colima/references/security_socket.md",
        include_str!("../../resources/skills/colima/references/security_socket.md"),
    ),
    (
        "skills/diagnostics/SKILL.md",
        include_str!("../../resources/skills/diagnostics/SKILL.md"),
    ),
    (
        "skills/diagnostics/references/process_locks.md",
        include_str!("../../resources/skills/diagnostics/references/process_locks.md"),
    ),
    (
        "skills/diagnostics/references/read_logs.md",
        include_str!("../../resources/skills/diagnostics/references/read_logs.md"),
    ),
    (
        "skills/docker/SKILL.md",
        include_str!("../../resources/skills/docker/SKILL.md"),
    ),
    (
        "skills/docker/references/network_isolation.md",
        include_str!("../../resources/skills/docker/references/network_isolation.md"),
    ),
    (
        "skills/docker/references/rootless_security.md",
        include_str!("../../resources/skills/docker/references/rootless_security.md"),
    ),
    (
        "skills/docker_compose/SKILL.md",
        include_str!("../../resources/skills/docker_compose/SKILL.md"),
    ),
    (
        "skills/docker_compose/references/secrets_management.md",
        include_str!("../../resources/skills/docker_compose/references/secrets_management.md"),
    ),
    (
        "skills/kubernetes/SKILL.md",
        include_str!("../../resources/skills/kubernetes/SKILL.md"),
    ),
    (
        "skills/kubernetes/references/colima_k3s_quirks.md",
        include_str!("../../resources/skills/kubernetes/references/colima_k3s_quirks.md"),
    ),
    (
        "skills/kubernetes/references/rbac_security.md",
        include_str!("../../resources/skills/kubernetes/references/rbac_security.md"),
    ),
    (
        "skills/limavm/SKILL.md",
        include_str!("../../resources/skills/limavm/SKILL.md"),
    ),
    (
        "skills/limavm/references/vm_security.md",
        include_str!("../../resources/skills/limavm/references/vm_security.md"),
    ),
    (
        "skills/security/SKILL.md",
        include_str!("../../resources/skills/security/SKILL.md"),
    ),
    // Knowledge base articles (en)
    (
        "kb/en/common-errors.md",
        include_str!("../../resources/kb/en/common-errors.md"),
    ),
    (
        "kb/en/honeypot-cowrie.md",
        include_str!("../../resources/kb/en/honeypot-cowrie.md"),
    ),
    (
        "kb/en/honeypot-logs.md",
        include_str!("../../resources/kb/en/honeypot-logs.md"),
    ),
    (
        "kb/en/honeypot-opencanary.md",
        include_str!("../../resources/kb/en/honeypot-opencanary.md"),
    ),
    (
        "kb/en/honeypot-overview.md",
        include_str!("../../resources/kb/en/honeypot-overview.md"),
    ),
    (
        "kb/en/image-hardening.md",
        include_str!("../../resources/kb/en/image-hardening.md"),
    ),
    (
        "kb/en/install-colima.md",
        include_str!("../../resources/kb/en/install-colima.md"),
    ),
    (
        "kb/en/install-docker-cli.md",
        include_str!("../../resources/kb/en/install-docker-cli.md"),
    ),
    (
        "kb/en/install-falco.md",
        include_str!("../../resources/kb/en/install-falco.md"),
    ),
    (
        "kb/en/install-kubectl.md",
        include_str!("../../resources/kb/en/install-kubectl.md"),
    ),
    (
        "kb/en/install-trivy.md",
        include_str!("../../resources/kb/en/install-trivy.md"),
    ),
    (
        "kb/en/performance-tuning.md",
        include_str!("../../resources/kb/en/performance-tuning.md"),
    ),
    (
        "kb/en/start-colima.md",
        include_str!("../../resources/kb/en/start-colima.md"),
    ),
    // Knowledge base articles (ja)
    (
        "kb/ja/common-errors.md",
        include_str!("../../resources/kb/ja/common-errors.md"),
    ),
    (
        "kb/ja/honeypot-cowrie.md",
        include_str!("../../resources/kb/ja/honeypot-cowrie.md"),
    ),
    (
        "kb/ja/honeypot-logs.md",
        include_str!("../../resources/kb/ja/honeypot-logs.md"),
    ),
    (
        "kb/ja/honeypot-opencanary.md",
        include_str!("../../resources/kb/ja/honeypot-opencanary.md"),
    ),
    (
        "kb/ja/honeypot-overview.md",
        include_str!("../../resources/kb/ja/honeypot-overview.md"),
    ),
    (
        "kb/ja/image-hardening.md",
        include_str!("../../resources/kb/ja/image-hardening.md"),
    ),
    (
        "kb/ja/install-colima.md",
        include_str!("../../resources/kb/ja/install-colima.md"),
    ),
    (
        "kb/ja/install-docker-cli.md",
        include_str!("../../resources/kb/ja/install-docker-cli.md"),
    ),
    (
        "kb/ja/install-falco.md",
        include_str!("../../resources/kb/ja/install-falco.md"),
    ),
    (
        "kb/ja/install-kubectl.md",
        include_str!("../../resources/kb/ja/install-kubectl.md"),
    ),
    (
        "kb/ja/install-trivy.md",
        include_str!("../../resources/kb/ja/install-trivy.md"),
    ),
    (
        "kb/ja/performance-tuning.md",
        include_str!("../../resources/kb/ja/performance-tuning.md"),
    ),
    (
        "kb/ja/start-colima.md",
        include_str!("../../resources/kb/ja/start-colima.md"),
    ),
    // Knowledge base articles (vi)
    (
        "kb/vi/common-errors.md",
        include_str!("../../resources/kb/vi/common-errors.md"),
    ),
    (
        "kb/vi/honeypot-cowrie.md",
        include_str!("../../resources/kb/vi/honeypot-cowrie.md"),
    ),
    (
        "kb/vi/honeypot-logs.md",
        include_str!("../../resources/kb/vi/honeypot-logs.md"),
    ),
    (
        "kb/vi/honeypot-opencanary.md",
        include_str!("../../resources/kb/vi/honeypot-opencanary.md"),
    ),
    (
        "kb/vi/honeypot-overview.md",
        include_str!("../../resources/kb/vi/honeypot-overview.md"),
    ),
    (
        "kb/vi/image-hardening.md",
        include_str!("../../resources/kb/vi/image-hardening.md"),
    ),
    (
        "kb/vi/install-colima.md",
        include_str!("../../resources/kb/vi/install-colima.md"),
    ),
    (
        "kb/vi/install-docker-cli.md",
        include_str!("../../resources/kb/vi/install-docker-cli.md"),
    ),
    (
        "kb/vi/install-falco.md",
        include_str!("../../resources/kb/vi/install-falco.md"),
    ),
    (
        "kb/vi/install-kubectl.md",
        include_str!("../../resources/kb/vi/install-kubectl.md"),
    ),
    (
        "kb/vi/install-trivy.md",
        include_str!("../../resources/kb/vi/install-trivy.md"),
    ),
    (
        "kb/vi/performance-tuning.md",
        include_str!("../../resources/kb/vi/performance-tuning.md"),
    ),
    (
        "kb/vi/start-colima.md",
        include_str!("../../resources/kb/vi/start-colima.md"),
    ),
    // Knowledge base articles (zh)
    (
        "kb/zh/common-errors.md",
        include_str!("../../resources/kb/zh/common-errors.md"),
    ),
    (
        "kb/zh/honeypot-cowrie.md",
        include_str!("../../resources/kb/zh/honeypot-cowrie.md"),
    ),
    (
        "kb/zh/honeypot-logs.md",
        include_str!("../../resources/kb/zh/honeypot-logs.md"),
    ),
    (
        "kb/zh/honeypot-opencanary.md",
        include_str!("../../resources/kb/zh/honeypot-opencanary.md"),
    ),
    (
        "kb/zh/honeypot-overview.md",
        include_str!("../../resources/kb/zh/honeypot-overview.md"),
    ),
    (
        "kb/zh/image-hardening.md",
        include_str!("../../resources/kb/zh/image-hardening.md"),
    ),
    (
        "kb/zh/install-colima.md",
        include_str!("../../resources/kb/zh/install-colima.md"),
    ),
    (
        "kb/zh/install-docker-cli.md",
        include_str!("../../resources/kb/zh/install-docker-cli.md"),
    ),
    (
        "kb/zh/install-falco.md",
        include_str!("../../resources/kb/zh/install-falco.md"),
    ),
    (
        "kb/zh/install-kubectl.md",
        include_str!("../../resources/kb/zh/install-kubectl.md"),
    ),
    (
        "kb/zh/install-trivy.md",
        include_str!("../../resources/kb/zh/install-trivy.md"),
    ),
    (
        "kb/zh/performance-tuning.md",
        include_str!("../../resources/kb/zh/performance-tuning.md"),
    ),
    (
        "kb/zh/start-colima.md",
        include_str!("../../resources/kb/zh/start-colima.md"),
    ),
];

fn is_valid_resources_dir(dir: &Path) -> bool {
    dir.is_dir() && dir.join("system_prompt.md").is_file()
}

/// Resolve an on-disk `resources` directory if one exists (macOS/Linux app bundle
/// next to `current_exe()`, or `src-tauri/resources` / `resources` in development).
pub fn resolve_resources_dir_on_disk() -> Option<PathBuf> {
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        // macOS `.app/Contents/MacOS/<bin>` -> `.app/Contents/Resources/resources`
        if let Some(contents_dir) = exe_dir.parent() {
            let macos_bundled = contents_dir.join("Resources").join("resources");
            if is_valid_resources_dir(&macos_bundled) {
                return Some(macos_bundled);
            }

            // Linux `.deb` / `.rpm` / AppImage `/usr/bin/<bin>` -> `/usr/lib/<pkg>/resources`
            for pkg_dir in ["colima-ui", "ColimaUI", "colima_ui"] {
                let linux_lib = contents_dir.join("lib").join(pkg_dir).join("resources");
                if is_valid_resources_dir(&linux_lib) {
                    return Some(linux_lib);
                }
            }
        }

        // Sibling `resources/` next to executable
        let sibling = exe_dir.join("resources");
        if is_valid_resources_dir(&sibling) {
            return Some(sibling);
        }
    }

    for dev_candidate in ["src-tauri/resources", "resources"] {
        let p = PathBuf::from(dev_candidate);
        if is_valid_resources_dir(&p) {
            return Some(p);
        }
    }

    None
}

fn lookup_embedded(normalized_rel: &str) -> Option<&'static str> {
    EMBEDDED_MARKDOWN_RESOURCES
        .iter()
        .find_map(|(key, content)| (*key == normalized_rel).then_some(*content))
}

fn append_user_plugins_section(context: &mut String, home_dir: Option<PathBuf>) {
    let Some(home) = home_dir else {
        return;
    };
    let plugins_file = home.join(".colima-ui").join("plugins.yaml");
    let Ok(content) = std::fs::read_to_string(&plugins_file) else {
        return;
    };
    let Ok(config) = serde_yml::from_str::<PluginsConfig>(&content) else {
        return;
    };
    if config.plugins.is_empty() {
        return;
    }

    context.push_str("## Custom User Plugins / Skills\n\n");
    context.push_str(
        "The user has defined the following custom skills/shortcuts in their `~/.colima-ui/plugins.yaml`. You MUST prioritize them when applicable.\n\n",
    );

    for (name, plugin) in config.plugins {
        context.push_str(&format!("### {name}\n"));
        context.push_str(&format!("- **Description**: {}\n", plugin.description));

        if let Some(prompt) = plugin.prompt {
            context.push_str(&format!("- **Prompt/Trigger**: {prompt}\n"));
        }

        if let Some(cmd) = plugin.command {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if !parts.is_empty() {
                let c = parts[0];
                let a: Vec<String> = parts[1..].iter().map(|s| format!("\"{s}\"")).collect();
                let args_str = format!("[{}]", a.join(", "));
                context.push_str(&format!(
                    "- **Action**: `{cmd}` (Run via `[EVENT_APPROVE: cli-exec | {{\"command\": \"{c}\", \"args\": {args_str}}}]`)\n"
                ));
            }
        }
        context.push('\n');
    }
}

/// Build the full AI application context directly from embedded resources + optional `~/.colima-ui/plugins.yaml`.
pub fn build_embedded_app_context(home_dir: Option<PathBuf>) -> String {
    let mut context = String::new();

    for key in ["system_prompt.md", "router.md", "skills/colima_ui/SKILL.md"] {
        if let Some(text) = lookup_embedded(key) {
            context.push_str(text);
            context.push_str("\n\n");
        }
    }

    append_user_plugins_section(&mut context, home_dir);
    context
}

/// Load the ColimaUI AI system context: prefers live disk `resources/` when present,
/// otherwise falls back to the compile-time embedded skills & prompts.
pub fn load_app_context(home_dir: Option<PathBuf>) -> String {
    if let Some(dir) = resolve_resources_dir_on_disk() {
        build_app_context_from_dir(&dir, home_dir)
    } else {
        build_embedded_app_context(home_dir)
    }
}

/// Validate and normalize a resource-relative path, rejecting `..` traversal,
/// backslashes, NUL bytes, and absolute paths outside `/resources/`.
pub fn normalize_resource_rel_path(rel_path: &str) -> Result<String, String> {
    let trimmed = rel_path.trim();
    if trimmed.is_empty() {
        return Err("Reference path must not be empty".to_string());
    }
    if trimmed.contains('\0') || trimmed.contains('\\') {
        return Err("Invalid characters in reference path".to_string());
    }

    // Reject absolute filesystem paths unless explicitly prefixed with `/resources/`.
    if trimmed.starts_with('/') && !trimmed.starts_with("/resources/") {
        return Err("Access outside resources directory is denied".to_string());
    }

    let raw_path = Path::new(trimmed);
    for comp in raw_path.components() {
        if matches!(comp, Component::ParentDir | Component::Prefix(_)) {
            return Err("Path traversal ('..') outside resources directory is denied".to_string());
        }
    }

    let stripped = trimmed.trim_start_matches("./").trim_start_matches('/');
    let stripped = stripped
        .strip_prefix("resources/")
        .unwrap_or(stripped)
        .trim_start_matches('/');

    let mut segments = Vec::new();
    for comp in Path::new(stripped).components() {
        match comp {
            Component::Normal(seg) => {
                let Some(s) = seg.to_str() else {
                    return Err("Reference path must be valid UTF-8".to_string());
                };
                if !s.is_empty() {
                    segments.push(s);
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(
                    "Path traversal ('..') outside resources directory is denied".to_string(),
                );
            }
        }
    }

    if segments.is_empty() {
        return Err("Reference path must name a file inside resources".to_string());
    }

    Ok(segments.join("/"))
}

/// Read any skill or reference Markdown file by relative path, trying the on-disk
/// `resources/` directory first (when present) and falling back to compile-time
/// embedded resources.
pub fn read_reference_any(rel_path: &str) -> Result<String, String> {
    let normalized = normalize_resource_rel_path(rel_path)?;

    if let Some(content) = resolve_resources_dir_on_disk()
        .and_then(|dir| read_reference_from_dir(&dir, &normalized).ok())
    {
        return Ok(content);
    }

    lookup_embedded(&normalized)
        .map(str::to_string)
        .ok_or_else(|| format!("Reference resource not found: {normalized}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_table_resolves_core_prompts_skills_and_references() {
        for required in [
            "system_prompt.md",
            "router.md",
            "skills/colima_ui/SKILL.md",
            "skills/colima/SKILL.md",
            "skills/colima/references/mount_volumes.md",
        ] {
            let embedded = lookup_embedded(required)
                .unwrap_or_else(|| panic!("missing embedded resource: {required}"));
            assert!(
                !embedded.trim().is_empty(),
                "embedded resource {required} must not be empty"
            );

            let via_any = read_reference_any(required)
                .unwrap_or_else(|e| panic!("read_reference_any({required}) failed: {e}"));
            assert_eq!(via_any, embedded);
        }

        // Prefixed forms (`resources/...` and `/resources/...`) normalize cleanly.
        let prefixed = read_reference_any("/resources/skills/colima/references/mount_volumes.md")
            .expect("prefixed /resources/... path resolves");
        assert!(prefixed.contains("mount") || !prefixed.is_empty());
    }

    #[test]
    fn embedded_context_matches_core_sections() {
        let embedded_ctx = build_embedded_app_context(None);
        assert!(embedded_ctx.contains("Context Prompt Router"));
        assert!(!embedded_ctx.trim().is_empty());

        let loaded_ctx = load_app_context(None);
        assert!(loaded_ctx.contains("Context Prompt Router"));
    }

    #[test]
    fn read_reference_any_rejects_traversal_and_outside_absolute_paths() {
        assert!(read_reference_any("../Cargo.toml").is_err());
        assert!(read_reference_any("skills/../../Cargo.toml").is_err());
        assert!(read_reference_any("/etc/passwd").is_err());
        assert!(read_reference_any("skills\\colima\\SKILL.md").is_err());
        assert!(read_reference_any("").is_err());
    }
}
