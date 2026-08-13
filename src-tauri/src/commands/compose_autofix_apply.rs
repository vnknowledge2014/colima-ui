//! Applying and undoing compose patches.
//!
//! Writing to a user's compose file is the only destructive thing this feature
//! does, so the write is bracketed by two things: a backup taken immediately
//! before it, and a re-run of the key check immediately before that.
//!
//! # Why the gate runs again here
//!
//! [`super::compose_autofix::key_diff_check`] already ran when the patch was
//! proposed. It runs again on apply because the content arriving at this
//! command came back from the client, and a command that trusts its caller to
//! have validated the payload is not validated at all. The second run costs a
//! YAML parse and closes the gap.
//!
//! # Backups live in app data, not beside the file
//!
//! A `.bak` next to the original lands in the user's repository, gets committed
//! by accident, and — for a compose file — may be picked up by tooling that
//! globs the directory. Backups go to the app's own data directory, keyed by
//! fix id, and are pruned after thirty days.

use serde::{Deserialize, Serialize};

/// A record of one applied fix, enough to undo it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRecord {
    pub fix_id: String,
    pub file_path: String,
    /// RFC 3339, UTC.
    pub applied_at: String,
    pub explanation: String,
    /// True once [`compose_autofix_undo`] has restored the original.
    pub undone: bool,
}

/// Backups older than this are removed on the next apply. Long enough to notice
/// a bad fix, short enough not to accumulate a user's configuration history
/// indefinitely.
const BACKUP_RETENTION_DAYS: i64 = 30;

/// Recognise the file names Docker itself treats as a Dockerfile.
///
/// Deliberately name-based rather than content-sniffing: which gate protects a
/// file must not depend on how the file happens to be broken.
pub fn is_dockerfile(path: &str) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name == "dockerfile"
        || name == "containerfile"
        || name.starts_with("dockerfile.")
        || name.ends_with(".dockerfile")
}

fn backup_dir() -> std::path::PathBuf {
    crate::path_util::app_data_dir().join("compose-backups")
}

fn record_path(fix_id: &str) -> std::path::PathBuf {
    backup_dir().join(format!("{}.json", fix_id))
}

fn backup_path(fix_id: &str) -> std::path::PathBuf {
    backup_dir().join(format!("{}.bak", fix_id))
}

/// Apply a patch: check it, back up the current file, then write.
///
/// `declared_removals` must accompany the content, because the check cannot
/// distinguish an intended key rename from an accidental deletion without it.
#[tauri::command]
pub async fn compose_autofix_apply(
    file_path: String,
    new_content: String,
    explanation: String,
    declared_removals: Vec<String>,
) -> Result<FixRecord, crate::error::ColimaError> {
    let current = std::fs::read_to_string(&file_path)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot read compose file: {}", e)))?;

    // The gate, run against what is on disk right now rather than against
    // whatever the file looked like when the patch was proposed.
    //
    // Which gate depends on the file. A Dockerfile is not YAML, and the YAML
    // check answers "unparseable original, nothing to protect" for one — so
    // sending a Dockerfile through it would wave every patch past unexamined.
    let refusal = if is_dockerfile(&file_path) {
        super::security_autofix::instruction_diff_check(&current, &new_content, &declared_removals)
            .err()
            .map(|missing| {
                missing
                    .into_iter()
                    .map(|m| format!("{} {}", m.keyword, m.argument))
                    .collect::<Vec<_>>()
            })
    } else {
        super::compose_autofix::key_diff_check(&current, &new_content, &declared_removals)
            .err()
            .map(|missing| missing.into_iter().map(|m| m.path).collect::<Vec<_>>())
    };

    if let Some(removed) = refusal {
        return Err(crate::error::ColimaError::from(format!(
            "Refused: this patch removes content it did not declare: {}",
            removed.join(", ")
        )));
    }

    let dir = backup_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot create backup dir: {}", e)))?;

    let now = chrono::Utc::now();
    let fix_id = format!("{}-{}", now.format("%Y%m%dT%H%M%S"), std::process::id());

    // Backup first. A failure here must abort before the original is touched.
    std::fs::write(backup_path(&fix_id), &current)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot write backup: {}", e)))?;

    std::fs::write(&file_path, &new_content).map_err(|e| {
        crate::error::ColimaError::from(format!("Cannot write compose file: {}", e))
    })?;

    let record = FixRecord {
        fix_id: fix_id.clone(),
        file_path,
        applied_at: now.to_rfc3339(),
        explanation,
        undone: false,
    };
    write_record(&record)?;
    prune_old_backups();

    Ok(record)
}

/// Restore the file to exactly what it was before the fix.
///
/// The backup is the bytes that were read off disk, so a restore is
/// byte-identical rather than a re-serialisation — which is what makes undo
/// safe for a file full of comments and formatting the product cannot model.
#[tauri::command]
pub async fn compose_autofix_undo(fix_id: String) -> Result<FixRecord, crate::error::ColimaError> {
    if fix_id.contains(['/', '\\']) || fix_id.contains("..") {
        return Err(crate::error::ColimaError::from(
            "Invalid fix id".to_string(),
        ));
    }

    let mut record: FixRecord = serde_json::from_str(
        &std::fs::read_to_string(record_path(&fix_id))
            .map_err(|_| crate::error::ColimaError::from("No such fix".to_string()))?,
    )
    .map_err(|e| crate::error::ColimaError::from(format!("Corrupt fix record: {}", e)))?;

    if record.undone {
        return Err(crate::error::ColimaError::from(
            "This fix has already been undone".to_string(),
        ));
    }

    let original = std::fs::read(backup_path(&fix_id)).map_err(|e| {
        crate::error::ColimaError::from(format!("Backup missing or unreadable: {}", e))
    })?;
    std::fs::write(&record.file_path, original).map_err(|e| {
        crate::error::ColimaError::from(format!("Cannot restore compose file: {}", e))
    })?;

    record.undone = true;
    write_record(&record)?;
    Ok(record)
}

/// Applied fixes, newest first.
///
/// Separate from the command so the activity feed can read the same records
/// without going through Tauri — one reader, so the panel and the timeline can
/// never disagree about what was applied.
pub fn history_records() -> Result<Vec<FixRecord>, String> {
    let dir = backup_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut records: Vec<FixRecord> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot read history: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|raw| serde_json::from_str::<FixRecord>(&raw).ok())
        .collect();
    records.sort_by(|a, b| b.applied_at.cmp(&a.applied_at));
    Ok(records)
}

#[tauri::command]
pub async fn compose_autofix_history() -> Result<Vec<FixRecord>, crate::error::ColimaError> {
    history_records().map_err(crate::error::ColimaError::from)
}

fn write_record(record: &FixRecord) -> Result<(), crate::error::ColimaError> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot encode fix record: {}", e)))?;
    std::fs::write(record_path(&record.fix_id), json)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot write fix record: {}", e)))
}

/// Drop backups past the retention window.
///
/// Best effort by design: failing to prune is not a reason to fail an apply the
/// user asked for.
fn prune_old_backups() {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(BACKUP_RETENTION_DAYS);
    let Ok(entries) = std::fs::read_dir(backup_dir()) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<FixRecord>(&raw) else {
            continue;
        };
        let Ok(applied) = chrono::DateTime::parse_from_rfc3339(&record.applied_at) else {
            continue;
        };
        if applied.with_timezone(&chrono::Utc) < cutoff {
            let _ = std::fs::remove_file(backup_path(&record.fix_id));
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Apply then undo must return the file byte-for-byte, comments and all.
    #[tokio::test]
    async fn undo_restores_the_file_byte_identically() {
        let dir = std::env::temp_dir().join(format!("colima-ui-autofix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("compose.yml");

        // Trailing spaces and a comment: the things a YAML round-trip destroys.
        let original = "# my stack   \nservices:\n  web:\n    image: nginx\n\n";
        std::fs::write(&file, original).unwrap();
        let path = file.to_string_lossy().to_string();

        let patched = format!("{}volumes:\n  data:\n", original);
        let record = compose_autofix_apply(
            path.clone(),
            patched.clone(),
            "declared volume".to_string(),
            vec![],
        )
        .await
        .expect("apply");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), patched);

        compose_autofix_undo(record.fix_id.clone()).await.expect("undo");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            original.as_bytes(),
            "undo must be byte-identical, not a re-serialisation"
        );

        // A second undo is refused rather than silently re-restoring.
        assert!(compose_autofix_undo(record.fix_id).await.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Defence in depth: the client is not trusted to have run the gate.
    #[tokio::test]
    async fn apply_refuses_a_patch_that_drops_a_block() {
        let dir =
            std::env::temp_dir().join(format!("colima-ui-autofix-gate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("compose.yml");

        let original = "services:\n  db:\n    image: postgres\n    environment:\n      PASSWORD: hunter2\n";
        std::fs::write(&file, original).unwrap();

        // Validates perfectly; takes the credentials with it.
        let hostile = "services:\n  db:\n    image: postgres\n";
        let err = compose_autofix_apply(
            file.to_string_lossy().to_string(),
            hostile.to_string(),
            "hostile".to_string(),
            vec![],
        )
        .await
        .expect_err("must be refused");
        assert!(format!("{:?}", err).contains("environment"));

        // And the file on disk is untouched.
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Dockerfile gate must actually engage. Run through the YAML check, a
    /// broken-looking Dockerfile parses as nothing and every patch sails past.
    #[tokio::test]
    async fn apply_refuses_a_dockerfile_patch_that_drops_an_instruction() {
        let dir = std::env::temp_dir().join(format!("colima-ui-sec-gate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Dockerfile");

        let original = "FROM alpine:3.19\nCOPY ./app /srv/app\nUSER appuser\n";
        std::fs::write(&file, original).unwrap();

        let hostile = "FROM alpine:3.19\nUSER appuser\n";
        let err = compose_autofix_apply(
            file.to_string_lossy().to_string(),
            hostile.to_string(),
            "hostile".to_string(),
            vec![],
        )
        .await
        .expect_err("must be refused");
        assert!(format!("{:?}", err).contains("COPY"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dockerfile_names_are_recognised() {
        for name in ["Dockerfile", "dockerfile", "Containerfile", "Dockerfile.dev", "api.dockerfile"] {
            assert!(is_dockerfile(&format!("/tmp/{}", name)), "{}", name);
        }
        for name in ["compose.yml", "docker-compose.yaml", "notes.txt"] {
            assert!(!is_dockerfile(&format!("/tmp/{}", name)), "{}", name);
        }
    }

    #[tokio::test]
    async fn a_traversing_fix_id_is_rejected() {
        assert!(compose_autofix_undo("../../etc/passwd".to_string())
            .await
            .is_err());
    }
}
