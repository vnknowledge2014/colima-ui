//! Services declared in compose files, as opposed to containers that exist.
//!
//! `docker ps` only knows about containers that were created. A compose project
//! that is down, or half up, has services the engine has never heard of — and
//! those are exactly the ones a user is looking for when something did not come
//! up. Reading the project's own compose files is the only way to see them.
//!
//! Kept separate from `topology`: this module knows YAML and the filesystem,
//! that one knows how to assemble a graph. Neither needs the other's problems.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A service as written in a compose file, before anything runs it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeService {
    pub project: String,
    pub name: String,
    pub image: String,
    /// Service names this one waits for, in the same project.
    pub depends_on: Vec<String>,
}

/// Compose files are configuration, not data. A file past this size is not one,
/// and reading it would cost far more than the graph it feeds.
const MAX_COMPOSE_BYTES: u64 = 2 * 1024 * 1024;

/// The subset of the compose schema this module cares about.
///
/// Everything else is ignored rather than rejected — an unknown key must never
/// stop the whole project from being read.
#[derive(Debug, Deserialize)]
struct ComposeFile {
    #[serde(default)]
    services: BTreeMap<String, ComposeServiceSpec>,
}

#[derive(Debug, Deserialize)]
struct ComposeServiceSpec {
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    depends_on: Option<serde_yml::Value>,
}

/// Read `depends_on` in both shapes the compose spec allows.
///
/// The short form is a list of names; the long form is a map keyed by name with
/// a condition. Handling only one of them would silently drop every dependency
/// in projects that use the other.
fn parse_depends_on(value: Option<&serde_yml::Value>) -> Vec<String> {
    match value {
        Some(serde_yml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        Some(serde_yml::Value::Mapping(map)) => map
            .iter()
            .filter_map(|(k, _)| k.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse one compose file's services. Returns None when it is not readable or
/// not a compose file at all.
fn parse_file(path: &str) -> Option<Vec<(String, ComposeServiceSpec)>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_COMPOSE_BYTES {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed: ComposeFile = serde_yml::from_str(&raw).ok()?;
    Some(parsed.services.into_iter().collect())
}

/// Services declared across every project's compose files.
///
/// Paths come from `docker compose ls`, which reports what the user's own
/// daemon recorded when the project was created — the same source already
/// trusted for container names, images and mounts. Each file is read
/// best-effort: one unreadable or malformed file costs that file's services and
/// produces a warning, never the whole graph.
///
/// Later files in a project override earlier ones, matching how compose merges
/// an override file over a base. `extends` and `include` are not followed —
/// they would mean resolving arbitrary paths from inside the file, which is a
/// different trust question than reading what the daemon already pointed at.
pub fn declared_services(
    projects: &[crate::commands::compose::ComposeProject],
) -> (Vec<ComposeService>, Vec<String>) {
    let mut services: Vec<ComposeService> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for project in projects {
        // A project with no recorded config files is not an error: it can be
        // started from a directory that has since moved.
        if project.config_files.trim().is_empty() {
            continue;
        }

        let mut merged: BTreeMap<String, ComposeServiceSpec> = BTreeMap::new();
        let mut read_any = false;

        for path in project.config_files.split(',').map(str::trim) {
            if path.is_empty() {
                continue;
            }
            match parse_file(path) {
                Some(entries) => {
                    read_any = true;
                    merged.extend(entries);
                }
                None => warnings.push(format!(
                    "compose file unreadable for project '{}': {}",
                    project.name, path
                )),
            }
        }

        if !read_any {
            continue;
        }

        for (name, spec) in merged {
            services.push(ComposeService {
                project: project.name.clone(),
                image: spec.image.clone().unwrap_or_default(),
                depends_on: parse_depends_on(spec.depends_on.as_ref()),
                name,
            });
        }
    }

    (services, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::compose::ComposeProject;
    use std::io::Write;

    fn write_temp(dir: &std::path::Path, name: &str, body: &str) -> String {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path.to_string_lossy().to_string()
    }

    fn project(name: &str, files: &str) -> ComposeProject {
        ComposeProject {
            name: name.to_string(),
            status: String::new(),
            config_files: files.to_string(),
        }
    }

    /// Both `depends_on` spellings are legal compose; supporting one would drop
    /// every dependency in projects using the other.
    #[test]
    fn depends_on_reads_list_and_map_forms() {
        let list: serde_yml::Value = serde_yml::from_str("[db, cache]").unwrap();
        assert_eq!(parse_depends_on(Some(&list)), vec!["db", "cache"]);

        let map: serde_yml::Value =
            serde_yml::from_str("db:\n  condition: service_healthy\n").unwrap();
        assert_eq!(parse_depends_on(Some(&map)), vec!["db"]);

        assert!(parse_depends_on(None).is_empty());
    }

    #[test]
    fn services_and_dependencies_are_read() {
        let dir = std::env::temp_dir().join(format!("cui-compose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = write_temp(
            &dir,
            "compose.yml",
            "services:\n  api:\n    image: node:20\n    depends_on: [db]\n  db:\n    image: postgres:16\n",
        );

        let (services, warnings) = declared_services(&[project("shop", &file)]);
        assert!(warnings.is_empty());
        assert_eq!(services.len(), 2);
        let api = services.iter().find(|s| s.name == "api").unwrap();
        assert_eq!(api.image, "node:20");
        assert_eq!(api.depends_on, vec!["db"]);
        assert_eq!(api.project, "shop");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// An override file adds to and replaces the base, the way compose merges.
    #[test]
    fn later_files_override_earlier_ones() {
        let dir = std::env::temp_dir().join(format!("cui-compose-ovr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = write_temp(&dir, "base.yml", "services:\n  api:\n    image: node:18\n");
        let over = write_temp(
            &dir,
            "over.yml",
            "services:\n  api:\n    image: node:20\n  worker:\n    image: busybox\n",
        );

        let (services, _) = declared_services(&[project("shop", &format!("{},{}", base, over))]);
        assert_eq!(services.len(), 2);
        assert_eq!(
            services.iter().find(|s| s.name == "api").unwrap().image,
            "node:20"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A missing file must cost that file only — never the rest of the graph.
    #[test]
    fn unreadable_file_warns_without_failing() {
        let (services, warnings) =
            declared_services(&[project("gone", "/nonexistent/compose.yml")]);
        assert!(services.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("gone"));
    }

    #[test]
    fn project_without_config_files_is_skipped_silently() {
        let (services, warnings) = declared_services(&[project("bare", "")]);
        assert!(services.is_empty());
        assert!(warnings.is_empty());
    }
}
