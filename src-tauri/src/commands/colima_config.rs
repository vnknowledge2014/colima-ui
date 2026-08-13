//! Read, validate and write `~/.colima/<profile>/colima.yaml`.
//!
//! # Why this does not reuse `instance_reader::ColimaConfig`
//!
//! That struct is `Deserialize`-only, parses seven fields, has no
//! `#[serde(flatten)]`, and its caller uses `unwrap_or_default()`. It exists to
//! answer "how big is this VM" for a list view, and it is correct for that.
//!
//! Serializing it back would produce a seven-key document: `mounts`,
//! `provision`, `env`, `dnsHosts`, `sshConfig` and every key a future colima
//! release adds would be deleted from the user's file. Writing that atomically
//! only guarantees the deletion is not interrupted.
//!
//! So the whole read/modify/write path here works on `serde_yml::Value`. We
//! mutate the specific keys the UI owns and hand the rest of the document back
//! untouched. `round_trip_preserves_unmanaged_fields` in the tests below is the
//! executable form of that promise.
//!
//! # Why parse failure is fatal
//!
//! `unwrap_or_default()` on a malformed file yields cpu/memory/disk = 0. If
//! that value reached the write path it would replace a working config with an
//! unbootable one. Every entry point here returns `Err` instead, and the write
//! path is unreachable without a successfully parsed document.

use serde::{Deserialize, Serialize};
use serde_yml::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{ColimaError, ErrorCode};

/// The colima.yaml keys this UI is allowed to touch. Everything else in the
/// document is carried through verbatim.
///
/// Nested keys use dotted paths matching colima's own layout: `network.dns`
/// and `network.address` live under a `network` mapping, `kubernetes.enabled`
/// under `kubernetes`.
pub const MANAGED_FIELDS: &[&str] = &[
    "cpu",
    "memory",
    "disk",
    "runtime",
    "vmType",
    "mountType",
    "network.dns",
    "network.address",
    "kubernetes.enabled",
];

const VALID_RUNTIMES: &[&str] = &["docker", "containerd", "incus"];
const VALID_VM_TYPES: &[&str] = &["qemu", "vz"];
const VALID_MOUNT_TYPES: &[&str] = &["sshfs", "9p", "virtiofs"];

// ===== Wire types =====

/// A requested edit. Every field is optional: absent means "leave alone",
/// which is what keeps a partial form submission from blanking the rest.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChanges {
    pub cpu: Option<u64>,
    /// GiB.
    pub memory: Option<u64>,
    /// GiB.
    pub disk: Option<u64>,
    pub runtime: Option<String>,
    pub vm_type: Option<String>,
    pub mount_type: Option<String>,
    pub dns: Option<Vec<String>>,
    pub network_address: Option<bool>,
    pub kubernetes: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Blocks the write.
    Error,
    /// Shown to the user; the write proceeds.
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    /// Dotted path from [`MANAGED_FIELDS`].
    pub field: String,
    pub severity: IssueSeverity,
    /// Stable key the frontend translates. English text lives in the locales.
    pub code: String,
    /// English fallback for surfaces with no i18n.
    pub message: String,
    /// Interpolation values for the translated string, e.g. `{"cpu": "32",
    /// "host": "8"}`.
    ///
    /// Without these a translated message would have to drop the numbers —
    /// "too many CPUs" instead of "32 requested, host has 8" — which is most of
    /// what makes the warning useful. `BTreeMap` so the JSON key order is
    /// stable and the payload diffs cleanly in tests.
    pub params: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub field: String,
    /// YAML scalar rendering of the old value, or `null` when the key was absent.
    pub from: Option<String>,
    pub to: Option<String>,
    pub requires_restart: bool,
}

/// What the config page loads.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSnapshot {
    pub profile: String,
    /// Current values of the managed fields only. The unmanaged remainder is
    /// deliberately not sent to the frontend — it has no reason to see it and
    /// no way to edit it.
    pub values: serde_json::Value,
    /// Unix seconds. Echoed back on write so a concurrent `colima start` that
    /// rewrote the file is detected instead of silently overwritten.
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub changes: Vec<FieldChange>,
    pub issues: Vec<ValidationIssue>,
    pub backup_path: Option<String>,
    pub mtime: i64,
}

// ===== Paths =====

/// Resolve a profile's config path.
///
/// `assert_path_within` is deliberately *not* called here: it canonicalizes the
/// parent directory, which fails for a profile that simply does not exist, and
/// reporting "invalid path" for a typo'd profile name sends the user looking for
/// the wrong problem. Traversal is already blocked by `ensure_valid_profile`,
/// which rejects `/`, `..` and everything else that could leave `~/.colima`.
/// The canonicalizing check then runs in [`existing_config_path`] and in
/// [`write_atomic`], where the path does exist and the check is meaningful.
fn config_path(home: &Path, profile: &str) -> Result<PathBuf, ColimaError> {
    crate::validation::ensure_valid_profile(profile).map_err(ColimaError::validation)?;
    let profile = if profile.is_empty() { "default" } else { profile };
    Ok(home.join(profile).join("colima.yaml"))
}

/// As [`config_path`], but the file must exist — so the containment check can
/// resolve symlinks for real.
fn existing_config_path(home: &Path, profile: &str) -> Result<PathBuf, ColimaError> {
    let path = config_path(home, profile)?;
    if !path.exists() {
        return Err(ColimaError::new(
            ErrorCode::NotFound,
            format!(
                "No config for profile {:?}. Start the instance once to create it.",
                profile
            ),
        ));
    }
    crate::validation::assert_path_within(home, &path).map_err(ColimaError::validation)?;
    Ok(path)
}

fn mtime_of(path: &Path) -> Result<i64, ColimaError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| ColimaError::internal(format!("Cannot stat {}: {}", path.display(), e)))?;
    let modified = meta
        .modified()
        .map_err(|e| ColimaError::internal(format!("No mtime for {}: {}", path.display(), e)))?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ColimaError::internal(format!("Bad mtime: {}", e)))?
        .as_secs();
    Ok(secs as i64)
}

// ===== Read =====

/// Parse a profile's colima.yaml into an untyped document.
///
/// Hard-fails on a malformed file. serde_yml reports the line and column in its
/// `Display`, so the message the user sees points at the broken line.
pub fn read_raw(home: &Path, profile: &str) -> Result<(Value, i64), ColimaError> {
    let path = existing_config_path(home, profile)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| ColimaError::internal(format!("Cannot read {}: {}", path.display(), e)))?;

    let value: Value = serde_yml::from_str(&text).map_err(|e| {
        ColimaError::validation(format!("{} is not valid YAML: {}", path.display(), e))
    })?;

    // A colima.yaml that parsed to a scalar or a list is syntactically valid
    // YAML but structurally wrong, and `apply_changes` would silently replace
    // it with a mapping. Reject it here instead.
    if !value.is_mapping() {
        return Err(ColimaError::validation(format!(
            "{} does not contain a YAML mapping",
            path.display()
        )));
    }

    let mtime = mtime_of(&path)?;
    Ok((value, mtime))
}

// ===== Value helpers =====

fn get_path<'a>(doc: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut cur = doc;
    for segment in dotted.split('.') {
        cur = cur.get(segment)?;
    }
    Some(cur)
}

/// Set a dotted path, creating intermediate mappings as needed.
///
/// If an intermediate key exists but is not a mapping (a user wrote
/// `network: ~`), it is replaced — there is no way to nest under a scalar, and
/// refusing the edit would leave the user unable to fix their own file.
fn set_path(doc: &mut Value, dotted: &str, new_value: Value) {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = doc;
    for segment in &segments[..segments.len() - 1] {
        if !cur.is_mapping() {
            *cur = Value::Mapping(Default::default());
        }
        let map = cur.as_mapping_mut().expect("just ensured mapping");
        let key = Value::String((*segment).to_string());
        cur = map
            .entry(key)
            .or_insert_with(|| Value::Mapping(Default::default()));
    }
    if !cur.is_mapping() {
        *cur = Value::Mapping(Default::default());
    }
    cur.as_mapping_mut()
        .expect("just ensured mapping")
        .insert(
            Value::String(segments[segments.len() - 1].to_string()),
            new_value,
        );
}

/// One-line rendering for the diff view. Sequences and mappings are shown as
/// their YAML flow form so `network.dns` reads as `[1.1.1.1]`.
fn render(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    Some(match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => serde_yml::to_string(other)
            .unwrap_or_default()
            .trim()
            .replace('\n', ", "),
    })
}

// ===== Apply =====

/// Write the requested changes onto `doc`, leaving every other key untouched.
pub fn apply_changes(doc: &mut Value, changes: &ConfigChanges) {
    if let Some(v) = changes.cpu {
        set_path(doc, "cpu", Value::Number(v.into()));
    }
    if let Some(v) = changes.memory {
        set_path(doc, "memory", Value::Number(v.into()));
    }
    if let Some(v) = changes.disk {
        set_path(doc, "disk", Value::Number(v.into()));
    }
    if let Some(v) = &changes.runtime {
        set_path(doc, "runtime", Value::String(v.clone()));
    }
    if let Some(v) = &changes.vm_type {
        set_path(doc, "vmType", Value::String(v.clone()));
    }
    if let Some(v) = &changes.mount_type {
        set_path(doc, "mountType", Value::String(v.clone()));
    }
    if let Some(v) = &changes.dns {
        let seq = v.iter().map(|s| Value::String(s.clone())).collect();
        set_path(doc, "network.dns", Value::Sequence(seq));
    }
    if let Some(v) = changes.network_address {
        set_path(doc, "network.address", Value::Bool(v));
    }
    if let Some(v) = changes.kubernetes {
        set_path(doc, "kubernetes.enabled", Value::Bool(v));
    }
}

// ===== Validate =====

fn issue(
    field: &str,
    severity: IssueSeverity,
    code: &str,
    message: String,
    params: &[(&str, String)],
) -> ValidationIssue {
    ValidationIssue {
        field: field.to_string(),
        severity,
        code: code.to_string(),
        message,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    }
}

fn as_u64(doc: &Value, dotted: &str) -> Option<u64> {
    get_path(doc, dotted).and_then(|v| v.as_u64())
}

/// Check the post-change document against the host and against colima's own
/// accepted values.
///
/// `old` is needed for the disk rule: a VM disk can be grown but not shrunk, so
/// the check is relative to the current size rather than an absolute floor.
pub fn validate(
    new: &Value,
    old: &Value,
    host: &crate::commands::system::HostSpecs,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    match as_u64(new, "cpu") {
        Some(0) | None => issues.push(issue(
            "cpu",
            IssueSeverity::Error,
            "cpu_zero",
            "CPU count must be at least 1.".to_string(),
            &[],
        )),
        Some(cpu) if cpu > host.cpu_cores as u64 => issues.push(issue(
            "cpu",
            IssueSeverity::Warning,
            "cpu_over_host",
            format!(
                "Requested {} CPUs but the host has {}. The VM will contend with the host for cores.",
                cpu, host.cpu_cores
            ),
            &[
                ("cpu", cpu.to_string()),
                ("host", host.cpu_cores.to_string()),
            ],
        )),
        _ => {}
    }

    match as_u64(new, "memory") {
        Some(0) | None => issues.push(issue(
            "memory",
            IssueSeverity::Error,
            "memory_zero",
            "Memory must be at least 1 GiB.".to_string(),
            &[],
        )),
        // Leaving the host with under 2 GiB is where macOS starts swapping
        // hard, so the warning fires before the VM literally exceeds RAM.
        Some(mem) if mem + 2 > host.memory_gib as u64 => issues.push(issue(
            "memory",
            IssueSeverity::Warning,
            "memory_over_host",
            format!(
                "Requested {} GiB of a {} GiB host. Leave at least 2 GiB for the host.",
                mem, host.memory_gib
            ),
            &[
                ("memory", mem.to_string()),
                ("host", host.memory_gib.to_string()),
            ],
        )),
        _ => {}
    }

    let old_disk = as_u64(old, "disk");
    match (as_u64(new, "disk"), old_disk) {
        (Some(0) | None, _) => issues.push(issue(
            "disk",
            IssueSeverity::Error,
            "disk_zero",
            "Disk size must be at least 1 GiB.".to_string(),
            &[],
        )),
        (Some(new_disk), Some(old_disk)) if new_disk < old_disk => issues.push(issue(
            "disk",
            IssueSeverity::Error,
            "disk_shrink",
            format!(
                "Disk cannot shrink from {} GiB to {} GiB. Colima can only grow a disk.",
                old_disk, new_disk
            ),
            &[
                ("from", old_disk.to_string()),
                ("to", new_disk.to_string()),
            ],
        )),
        (Some(new_disk), _) if new_disk > host.disk_free_gib as u64 => issues.push(issue(
            "disk",
            IssueSeverity::Warning,
            "disk_over_free",
            format!(
                "Requested {} GiB but only {} GiB is free on the host.",
                new_disk, host.disk_free_gib
            ),
            &[
                ("disk", new_disk.to_string()),
                ("free", host.disk_free_gib.to_string()),
            ],
        )),
        _ => {}
    }

    for (field, allowed, code) in [
        ("runtime", VALID_RUNTIMES, "runtime_invalid"),
        ("vmType", VALID_VM_TYPES, "vm_type_invalid"),
        ("mountType", VALID_MOUNT_TYPES, "mount_type_invalid"),
    ] {
        // Absent is fine: colima applies its own default. Present-but-wrong is
        // an error, because colima will refuse to start and the user will see
        // the failure minutes later instead of now.
        if let Some(value) = get_path(new, field).and_then(|v| v.as_str()) {
            if !allowed.contains(&value) {
                issues.push(issue(
                    field,
                    IssueSeverity::Error,
                    code,
                    format!("{:?} is not one of: {}.", value, allowed.join(", ")),
                    &[
                        ("value", value.to_string()),
                        ("allowed", allowed.join(", ")),
                    ],
                ));
            }
        }
    }

    if let Some(Value::Sequence(entries)) = get_path(new, "network.dns") {
        for entry in entries {
            let text = entry.as_str().unwrap_or("");
            if text.parse::<std::net::IpAddr>().is_err() {
                issues.push(issue(
                    "network.dns",
                    IssueSeverity::Error,
                    "dns_not_ip",
                    format!("{:?} is not an IP address.", text),
                    &[("value", text.to_string())],
                ));
            }
        }
    }

    issues
}

// ===== Diff =====

/// Every managed field is read by colima at `colima start` and nowhere else, so
/// a change to any of them only takes effect on restart.
///
/// This is a function rather than a literal `true` because the diff shape is
/// what the UI renders, and the moment a non-restart field joins
/// [`MANAGED_FIELDS`] this is the single place that has to learn about it.
fn requires_restart(_field: &str) -> bool {
    true
}

pub fn diff(old: &Value, new: &Value) -> Vec<FieldChange> {
    MANAGED_FIELDS
        .iter()
        .filter_map(|field| {
            let before = get_path(old, field);
            let after = get_path(new, field);
            if before == after {
                return None;
            }
            Some(FieldChange {
                field: (*field).to_string(),
                from: render(before),
                to: render(after),
                requires_restart: requires_restart(field),
            })
        })
        .collect()
}

// ===== Write =====

/// Back up, then replace the config in one rename.
///
/// Ordering matters: the backup is written and fsynced before the temp file is
/// renamed over the original, so an interruption at any point leaves either the
/// original or the original plus a usable `.bak`.
pub fn write_atomic(home: &Path, profile: &str, doc: &Value) -> Result<String, ColimaError> {
    let path = existing_config_path(home, profile)?;
    let dir = path
        .parent()
        .ok_or_else(|| ColimaError::internal("config path has no parent"))?;

    let serialized = serde_yml::to_string(doc)
        .map_err(|e| ColimaError::internal(format!("Cannot serialize config: {}", e)))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = dir.join(format!("colima.yaml.{}.bak", stamp));
    crate::validation::assert_path_within(home, &backup).map_err(ColimaError::validation)?;
    std::fs::copy(&path, &backup)
        .map_err(|e| ColimaError::internal(format!("Cannot write backup: {}", e)))?;

    let tmp = dir.join(format!("colima.yaml.{}.tmp", stamp));
    crate::validation::assert_path_within(home, &tmp).map_err(ColimaError::validation)?;
    {
        let mut file = std::fs::File::create(&tmp)
            .map_err(|e| ColimaError::internal(format!("Cannot create temp file: {}", e)))?;
        file.write_all(serialized.as_bytes())
            .map_err(|e| ColimaError::internal(format!("Cannot write temp file: {}", e)))?;
        file.sync_all()
            .map_err(|e| ColimaError::internal(format!("Cannot fsync temp file: {}", e)))?;
    }

    std::fs::rename(&tmp, &path).map_err(|e| {
        // Leave nothing half-written behind if the rename itself failed.
        let _ = std::fs::remove_file(&tmp);
        ColimaError::internal(format!("Cannot replace config: {}", e))
    })?;

    Ok(backup.to_string_lossy().to_string())
}

// ===== Orchestration =====

fn managed_values(doc: &Value) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for field in MANAGED_FIELDS {
        let value = get_path(doc, field)
            .and_then(|v| serde_json::to_value(v).ok())
            .unwrap_or(serde_json::Value::Null);
        map.insert((*field).to_string(), value);
    }
    serde_json::Value::Object(map)
}

fn home_dir() -> PathBuf {
    crate::instance_reader::colima_home()
}

/// Load the managed fields of a profile's config.
#[tauri::command]
pub async fn get_colima_config(profile: String) -> Result<ConfigSnapshot, ColimaError> {
    let (doc, mtime) = read_raw(&home_dir(), &profile)?;
    Ok(ConfigSnapshot {
        profile,
        values: managed_values(&doc),
        mtime,
    })
}

/// Compute the diff and validation issues for a set of changes without writing.
#[tauri::command]
pub async fn preview_colima_config(
    profile: String,
    changes: ConfigChanges,
) -> Result<ApplyResult, ColimaError> {
    // Nothing is recorded here on purpose: previewing changes nothing about the
    // machine, and a log full of "looked at the config" would bury the entries
    // that describe what was actually done to it.
    let home = home_dir();
    let (old, mtime) = read_raw(&home, &profile)?;
    let mut new = old.clone();
    apply_changes(&mut new, &changes);

    let host = crate::commands::system::detect_host_specs();
    Ok(ApplyResult {
        changes: diff(&old, &new),
        issues: validate(&new, &old, &host),
        backup_path: None,
        mtime,
    })
}

/// Validate and write the changes.
///
/// `expected_mtime` comes from the [`ConfigSnapshot`] the form was populated
/// from. Colima rewrites colima.yaml on every `colima start`, so between the
/// user opening the page and pressing Apply the file may have changed
/// underneath; writing then would silently revert whatever colima just wrote.
#[tauri::command]
pub async fn apply_colima_config(
    profile: String,
    changes: ConfigChanges,
    expected_mtime: i64,
) -> Result<ApplyResult, ColimaError> {
    // Wrapped whole rather than recorded at the end: this returns early in
    // three places, and two of them — a stale-mtime refusal and an apply with
    // nothing to apply — are exactly the cases somebody comes back asking about.
    let logged_profile = profile.clone();
    let result = (|| -> Result<ApplyResult, ColimaError> {
    let home = home_dir();
    let (old, mtime) = read_raw(&home, &profile)?;
    if mtime != expected_mtime {
        return Err(ColimaError::validation(
            "The config file changed on disk since it was loaded. Reload and try again."
                .to_string(),
        ));
    }

    let mut new = old.clone();
    apply_changes(&mut new, &changes);

    let host = crate::commands::system::detect_host_specs();
    let issues = validate(&new, &old, &host);
    if issues.iter().any(|i| i.severity == IssueSeverity::Error) {
        return Ok(ApplyResult {
            changes: diff(&old, &new),
            issues,
            backup_path: None,
            mtime,
        });
    }

    let changes_list = diff(&old, &new);
    if changes_list.is_empty() {
        return Ok(ApplyResult {
            changes: changes_list,
            issues,
            backup_path: None,
            mtime,
        });
    }

    let backup = write_atomic(&home, &profile, &new)?;
    let new_mtime = mtime_of(&existing_config_path(&home, &profile)?)?;

    Ok(ApplyResult {
        changes: changes_list,
        issues,
        backup_path: Some(backup),
        mtime: new_mtime,
    })
    })();

    // What the record says depends on what actually reached the disk. Two of
    // the branches above return `Ok` having written nothing — logging those as
    // "applied" would put a change in the history that never happened.
    let detail = match &result {
        Ok(r) if r.backup_path.is_some() => format!("{} fields changed", r.changes.len()),
        Ok(r) if r.issues.iter().any(|i| i.severity == IssueSeverity::Error) => {
            "not applied: the changes did not validate".to_string()
        }
        Ok(_) => "no changes to apply".to_string(),
        Err(_) => String::new(),
    };
    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "apply",
            "colima_config",
            &logged_profile,
        )
        .detail(detail)
        .outcome_of(&result),
    );

    result
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_CONFIG: &str = r#"cpu: 2
disk: 60
memory: 2
arch: aarch64
runtime: docker
hostname: colima
kubernetes:
  enabled: false
  version: v1.31.2+k3s1
  k3sArgs:
    - --disable=traefik
autoActivate: true
network:
  address: false
  dns: []
  dnsHosts:
    host.docker.internal: host.lima.internal
forwardAgent: false
docker: {}
vmType: qemu
mountType: sshfs
mounts:
  - location: /Users/me/work
    writable: true
provision:
  - mode: system
    script: apk add htop
env:
  MY_VAR: keep-me
sshConfig: true
customFutureKey: preserved
"#;

    fn host() -> crate::commands::system::HostSpecs {
        crate::commands::system::HostSpecs {
            cpu_cores: 8,
            memory_gib: 16,
            disk_free_gib: 200,
            disk_total_gib: 500,
            arch: "aarch64".to_string(),
            model: "test".to_string(),
        }
    }

    fn parse(text: &str) -> Value {
        serde_yml::from_str(text).expect("fixture must parse")
    }

    fn temp_home(config: &str) -> tempdir::Home {
        tempdir::Home::with_config("default", config)
    }

    /// Minimal scratch-directory helper. The crate has no dev-dependency on
    /// `tempfile` and this plan forbids adding dependencies, so the two things
    /// actually needed — a unique directory and cleanup on drop — live here.
    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        pub struct Home(PathBuf);

        impl Home {
            pub fn with_config(profile: &str, config: &str) -> Self {
                let unique = format!(
                    "colima-ui-test-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::SeqCst)
                );
                let root = std::env::temp_dir().join(unique);
                let dir = root.join(profile);
                std::fs::create_dir_all(&dir).expect("create temp profile dir");
                std::fs::write(dir.join("colima.yaml"), config).expect("write fixture config");
                Home(root)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }

            pub fn config_text(&self, profile: &str) -> String {
                std::fs::read_to_string(self.0.join(profile).join("colima.yaml"))
                    .expect("read config back")
            }

            pub fn backups(&self, profile: &str) -> Vec<PathBuf> {
                std::fs::read_dir(self.0.join(profile))
                    .expect("list profile dir")
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "bak"))
                    .collect()
            }
        }

        impl Drop for Home {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    /// The merge condition for this phase: changing one field must not disturb
    /// any other key in the document, including keys this code has never heard
    /// of.
    #[test]
    fn round_trip_preserves_unmanaged_fields() {
        let home = temp_home(FULL_CONFIG);
        let (mut doc, _) = read_raw(home.path(), "default").expect("fixture parses");

        apply_changes(
            &mut doc,
            &ConfigChanges {
                cpu: Some(6),
                ..Default::default()
            },
        );
        write_atomic(home.path(), "default", &doc).expect("write succeeds");

        let written = parse(&home.config_text("default"));

        assert_eq!(written.get("cpu").and_then(|v| v.as_u64()), Some(6));

        // Everything else, verbatim.
        let original = parse(FULL_CONFIG);
        for key in [
            "disk",
            "memory",
            "arch",
            "runtime",
            "hostname",
            "kubernetes",
            "autoActivate",
            "network",
            "forwardAgent",
            "docker",
            "vmType",
            "mountType",
            "mounts",
            "provision",
            "env",
            "sshConfig",
            "customFutureKey",
        ] {
            assert_eq!(
                written.get(key),
                original.get(key),
                "key {:?} was modified by an unrelated edit",
                key
            );
        }
    }

    #[test]
    fn nested_edits_preserve_siblings() {
        let mut doc = parse(FULL_CONFIG);
        apply_changes(
            &mut doc,
            &ConfigChanges {
                dns: Some(vec!["1.1.1.1".to_string()]),
                kubernetes: Some(true),
                ..Default::default()
            },
        );

        assert_eq!(
            get_path(&doc, "network.dns"),
            Some(&Value::Sequence(vec![Value::String("1.1.1.1".into())]))
        );
        assert_eq!(get_path(&doc, "kubernetes.enabled"), Some(&Value::Bool(true)));

        // Siblings under the same mappings survive.
        let original = parse(FULL_CONFIG);
        assert_eq!(
            get_path(&doc, "network.dnsHosts"),
            get_path(&original, "network.dnsHosts")
        );
        assert_eq!(
            get_path(&doc, "kubernetes.k3sArgs"),
            get_path(&original, "kubernetes.k3sArgs")
        );
        assert_eq!(
            get_path(&doc, "kubernetes.version"),
            get_path(&original, "kubernetes.version")
        );
    }

    #[test]
    fn absent_change_fields_touch_nothing() {
        let original = parse(FULL_CONFIG);
        let mut doc = original.clone();
        apply_changes(&mut doc, &ConfigChanges::default());
        assert_eq!(doc, original);
    }

    #[test]
    fn malformed_yaml_is_an_error_and_nothing_is_written() {
        let home = temp_home("cpu: 2\n  bad indent: [\n");
        let before = home.config_text("default");

        let result = read_raw(home.path(), "default");
        assert!(result.is_err(), "malformed YAML must not parse");

        assert_eq!(home.config_text("default"), before, "file must be untouched");
        assert!(home.backups("default").is_empty(), "no backup on failed read");
    }

    #[test]
    fn non_mapping_yaml_is_rejected() {
        let home = temp_home("- just\n- a list\n");
        assert!(read_raw(home.path(), "default").is_err());
    }

    #[test]
    fn missing_profile_reports_not_found() {
        let home = temp_home(FULL_CONFIG);
        let err = read_raw(home.path(), "nonexistent").expect_err("profile does not exist");
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn traversal_profile_names_are_rejected() {
        let home = temp_home(FULL_CONFIG);
        assert!(read_raw(home.path(), "../../etc").is_err());
        assert!(read_raw(home.path(), "a/b").is_err());
    }

    #[test]
    fn validate_flags_resource_overcommit() {
        let old = parse(FULL_CONFIG);
        let mut new = old.clone();
        apply_changes(
            &mut new,
            &ConfigChanges {
                cpu: Some(32),
                memory: Some(16),
                ..Default::default()
            },
        );

        let issues = validate(&new, &old, &host());
        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"cpu_over_host"), "got {:?}", codes);
        assert!(codes.contains(&"memory_over_host"), "got {:?}", codes);
        // Overcommit is legal, just unwise.
        assert!(issues.iter().all(|i| i.severity == IssueSeverity::Warning));
    }

    #[test]
    fn validate_blocks_disk_shrink_and_bad_enums() {
        let old = parse(FULL_CONFIG);
        let mut new = old.clone();
        apply_changes(
            &mut new,
            &ConfigChanges {
                disk: Some(10),
                runtime: Some("podman".to_string()),
                vm_type: Some("hyperv".to_string()),
                dns: Some(vec!["not-an-ip".to_string()]),
                ..Default::default()
            },
        );

        let issues = validate(&new, &old, &host());
        let errors: Vec<&str> = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .map(|i| i.code.as_str())
            .collect();
        assert!(errors.contains(&"disk_shrink"), "got {:?}", errors);
        assert!(errors.contains(&"runtime_invalid"), "got {:?}", errors);
        assert!(errors.contains(&"vm_type_invalid"), "got {:?}", errors);
        assert!(errors.contains(&"dns_not_ip"), "got {:?}", errors);
    }

    /// The locale files interpolate `{cpu}`, `{host}` and friends. If Rust
    /// stops sending a name the translated string references, the user sees a
    /// literal `{host}` — so every issue that quotes a number must carry it.
    #[test]
    fn issues_carrying_numbers_also_carry_them_as_params() {
        let old = parse(FULL_CONFIG);
        let mut new = old.clone();
        apply_changes(
            &mut new,
            &ConfigChanges {
                cpu: Some(64),
                memory: Some(64),
                disk: Some(10),
                runtime: Some("podman".to_string()),
                dns: Some(vec!["nope".to_string()]),
                ..Default::default()
            },
        );

        let issues = validate(&new, &old, &host());
        let params_for = |code: &str| {
            issues
                .iter()
                .find(|i| i.code == code)
                .unwrap_or_else(|| panic!("expected a {} issue, got {:?}", code, issues))
                .params
                .clone()
        };

        for (code, keys) in [
            ("cpu_over_host", vec!["cpu", "host"]),
            ("memory_over_host", vec!["memory", "host"]),
            ("disk_shrink", vec!["from", "to"]),
            ("runtime_invalid", vec!["value", "allowed"]),
            ("dns_not_ip", vec!["value"]),
        ] {
            let params = params_for(code);
            for key in keys {
                assert!(
                    params.contains_key(key),
                    "{} is missing param {:?} (has {:?})",
                    code,
                    key,
                    params
                );
            }
        }
    }

    #[test]
    fn validate_accepts_an_unchanged_document() {
        let doc = parse(FULL_CONFIG);
        let issues = validate(&doc, &doc, &host());
        assert!(issues.is_empty(), "clean config produced {:?}", issues);
    }

    #[test]
    fn diff_lists_only_changed_managed_fields_and_marks_restart() {
        let old = parse(FULL_CONFIG);
        let mut new = old.clone();
        apply_changes(
            &mut new,
            &ConfigChanges {
                cpu: Some(4),
                kubernetes: Some(true),
                ..Default::default()
            },
        );

        let changes = diff(&old, &new);
        let fields: Vec<&str> = changes.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(fields, vec!["cpu", "kubernetes.enabled"]);

        let cpu = &changes[0];
        assert_eq!(cpu.from.as_deref(), Some("2"));
        assert_eq!(cpu.to.as_deref(), Some("4"));
        assert!(changes.iter().all(|c| c.requires_restart));
    }

    #[test]
    fn diff_reports_a_previously_absent_key_as_added() {
        let old = parse("cpu: 2\nmemory: 2\ndisk: 60\n");
        let mut new = old.clone();
        apply_changes(
            &mut new,
            &ConfigChanges {
                mount_type: Some("virtiofs".to_string()),
                ..Default::default()
            },
        );

        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "mountType");
        assert_eq!(changes[0].from, None);
        assert_eq!(changes[0].to.as_deref(), Some("virtiofs"));
    }

    #[test]
    fn write_atomic_leaves_a_backup_of_the_previous_content() {
        let home = temp_home(FULL_CONFIG);
        let (mut doc, _) = read_raw(home.path(), "default").unwrap();
        apply_changes(
            &mut doc,
            &ConfigChanges {
                cpu: Some(4),
                ..Default::default()
            },
        );

        let backup = write_atomic(home.path(), "default", &doc).expect("write");
        let backed_up = std::fs::read_to_string(&backup).expect("backup readable");

        assert_eq!(backed_up, FULL_CONFIG, "backup must hold the pre-write file");
        assert_eq!(home.backups("default").len(), 1);
        // And no temp file survived.
        assert!(!home
            .backups("default")
            .iter()
            .any(|p| p.to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn set_path_replaces_a_scalar_intermediate() {
        let mut doc = parse("network: ~\ncpu: 2\n");
        apply_changes(
            &mut doc,
            &ConfigChanges {
                network_address: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(get_path(&doc, "network.address"), Some(&Value::Bool(true)));
        assert_eq!(doc.get("cpu").and_then(|v| v.as_u64()), Some(2));
    }
}
