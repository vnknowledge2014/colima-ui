//! Compose auto-fix: propose patches for a file `compose_diagnose` found broken.
//!
//! `compose_diagnose` answers *why is this file broken*. This module answers
//! *how do I fix it*, and it is deliberately far more conservative than that
//! sounds. A fixability gate run over a 20-file corpus put deterministic
//! coverage at roughly a third of real failures, so only three fix shapes are
//! offered as something to apply — declaring a referenced volume or network,
//! coercing a scalar into the list the schema demands, and replacing tab
//! indentation. Everything else is explanation, not surgery.
//!
//! # The check that matters
//!
//! [`key_diff_check`] is the gate, not a warning. A patch that quietly drops a
//! block still passes `docker compose config`, so syntax validation cannot
//! catch the one failure that would hurt: an `environment:` or `secrets:` block
//! disappearing takes the service's credentials with it and leaves a file that
//! validates perfectly. Every patch is compared key-by-key against the original
//! before it is ever shown, and a key that vanished without being declared is a
//! refusal.
//!
//! Declared removals are what make that check usable rather than merely strict.
//! Correcting a misspelled key genuinely removes `imge` — so a fixer that
//! renames a key must say so, and the check subtracts what was declared from
//! what it observed.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Where a patch came from. Shown to the user, because trust differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchSource {
    /// Derived from the error text by rule. No model, works offline.
    Deterministic,
    /// A Knowledge Bank entry that shipped a patch alongside its explanation.
    Kb,
    /// A scoped edit proposed by a model. Never a whole-file rewrite.
    Llm,
    /// Derived from a failing security rule. Same patch shape and the same
    /// apply path — a second apply path would be a second place for a fix to
    /// go wrong.
    Security,
}

/// A single proposed edit to a compose file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Stable within one proposal round; used by the UI to track selection.
    pub id: String,
    pub unified_diff: String,
    /// The full file as it would be written. Applying is a straight write.
    pub new_content: String,
    pub explanation: String,
    pub source: PatchSource,
    /// 0.0–1.0. Deterministic fixes are not automatically certain: declaring a
    /// volume the file already references is, correcting a typo is a guess.
    pub confidence: f32,
    /// False when the fix necessarily reflows the document and loses comments.
    /// Surfaced per patch because the product must not promise what YAML
    /// round-tripping cannot deliver.
    pub comments_preserved: bool,
    /// Keys this patch intends to remove, as dotted paths. Anything removed
    /// that is not listed here is a defect and blocks the patch.
    pub declared_removals: Vec<String>,
    /// The `compose_diagnose` category this patch responds to.
    pub category: String,
}

/// A key that disappeared without the patch admitting to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingKey {
    /// Dotted path, e.g. `services.web.environment`.
    pub path: String,
}

/// Collect every mapping key in a YAML document as a dotted path.
///
/// Sequence elements are indexed (`ports[0]`) so that a key nested inside a
/// list item is still reachable. Values are deliberately ignored: this check
/// exists to prove nothing was *dropped*, and comparing values would reject
/// every fix that changes one.
fn collect_key_paths(value: &serde_yml::Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_yml::Value::Mapping(map) => {
            for (k, v) in map {
                let key = match k {
                    serde_yml::Value::String(s) => s.clone(),
                    other => format!("{:?}", other),
                };
                let path = if prefix.is_empty() {
                    key
                } else {
                    format!("{}.{}", prefix, key)
                };
                out.insert(path.clone());
                collect_key_paths(v, &path, out);
            }
        }
        serde_yml::Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                collect_key_paths(item, &format!("{}[{}]", prefix, i), out);
            }
        }
        _ => {}
    }
}

/// Refuse any patch that removes a key it did not declare.
///
/// This is the only check that catches a patch silently deleting a block that
/// carries secrets, because such a file still validates cleanly. It runs before
/// a patch is displayed, not before it is applied — a patch the user should
/// never see is not a patch to warn about.
///
/// `declared_removals` are the paths the fixer says it is removing on purpose,
/// such as the misspelled key a rename replaces.
pub fn key_diff_check(
    old_yaml: &str,
    new_yaml: &str,
    declared_removals: &[String],
) -> Result<(), Vec<MissingKey>> {
    // An unparseable original means there are no keys to protect — a tab-indent
    // or unclosed-quote fix is exactly the case where the old file does not
    // parse, and refusing it here would block the fixes that work best.
    let old: serde_yml::Value = match serde_yml::from_str(old_yaml) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    // The new file failing to parse is a different matter: the patch produced
    // something worse than it was given.
    let new: serde_yml::Value = match serde_yml::from_str(new_yaml) {
        Ok(v) => v,
        Err(_) => {
            return Err(vec![MissingKey {
                path: "<entire document: patched file is not valid YAML>".to_string(),
            }])
        }
    };

    let mut old_keys = BTreeSet::new();
    let mut new_keys = BTreeSet::new();
    collect_key_paths(&old, "", &mut old_keys);
    collect_key_paths(&new, "", &mut new_keys);

    let declared: BTreeSet<&str> = declared_removals.iter().map(|s| s.as_str()).collect();

    let missing: Vec<MissingKey> = old_keys
        .difference(&new_keys)
        .filter(|path| !declared.contains(path.as_str()))
        // A declared removal takes its whole subtree with it. Listing every
        // descendant would make renaming a populated key unusable.
        .filter(|path| {
            !declared
                .iter()
                .any(|d| path.starts_with(&format!("{}.", d)) || path.starts_with(&format!("{}[", d)))
        })
        .map(|path| MissingKey { path: path.clone() })
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Render a unified diff between two versions of a file.
///
/// Compose files are small, so an O(n·m) longest-common-subsequence table is
/// cheaper than taking on a diff dependency for one screen of output.
pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();

    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = format!("--- {}\n+++ {}\n", path, path);
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push_str(&format!(" {}\n", a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push_str(&format!("-{}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+{}\n", b[j]));
            j += 1;
        }
    }
    for line in a.iter().skip(i) {
        out.push_str(&format!("-{}\n", line));
    }
    for line in b.iter().skip(j) {
        out.push_str(&format!("+{}\n", line));
    }
    out
}

/// Everything the UI needs to offer a fix, or to explain why none is offered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutofixProposal {
    /// Cumulative steps, each carrying the whole file as it would then stand.
    /// Applying step *n* means writing `patches[n].new_content`; the last entry
    /// is the complete fix.
    pub patches: Vec<Patch>,
    /// True when the file validates after the last patch.
    pub resolved: bool,
    /// What Docker still complains about when `resolved` is false. This is the
    /// honest half of the feature: the gate put deterministic coverage near a
    /// third, so stopping short is the common case, not the exception.
    pub remaining_error: String,
    /// Patches suppressed by [`key_diff_check`]. Non-empty here is a defect in
    /// a fixer, surfaced rather than swallowed.
    pub refused: Vec<MissingKey>,
}

/// Docker reports only the *first* undefined reference, so one pass fixes one
/// error and reveals the next. Five rounds clears the corpus with room to
/// spare, and bounds a fixer that fails to make progress.
const MAX_ROUNDS: usize = 5;

/// Propose deterministic fixes for a broken compose file, iterating to a fixed
/// point.
///
/// Nothing is written to the user's file. Each candidate is validated by
/// writing it beside the original under a temporary name — beside, because
/// `build:` contexts and `env_file:` paths resolve relative to the compose
/// file's own directory, so validating a copy elsewhere would report failures
/// the real file does not have.
#[tauri::command]
pub async fn compose_autofix_propose(
    file_path: String,
) -> Result<AutofixProposal, crate::error::ColimaError> {
    use crate::commands::compose_autofix_fixers as fixers;

    let original = std::fs::read_to_string(&file_path)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot read compose file: {}", e)))?;

    let scratch = ScratchFile::beside(&file_path)?;

    let mut current = original.clone();
    let mut patches: Vec<Patch> = vec![];
    let mut refused: Vec<MissingKey> = vec![];
    let mut remaining_error = String::new();
    let mut resolved = false;

    for round in 0..MAX_ROUNDS {
        scratch.write(&current)?;
        let validation =
            crate::commands::compose_diagnose::compose_validate(scratch.path_string()).await?;
        if validation.valid {
            resolved = true;
            break;
        }
        remaining_error = validation.raw_error.clone();

        let fix = match validation.category.as_str() {
            "undefined_reference" => fixers::fix_undefined_toplevel(&current, &validation.raw_error),
            "schema" => fixers::fix_scalar_to_list(&current, &validation.raw_error),
            "yaml_syntax" => fixers::fix_tabs(&current),
            // `missing_file`, `structure` and `other` are explained, never
            // edited: repairing them needs the author's intent, not the error.
            _ => None,
        };

        let Some(fix) = fix else { break };

        // A fixer that made no change would spin the loop to its bound while
        // proposing nothing.
        if fix.new_content == current {
            break;
        }

        // The gate. A patch that fails it is never shown, and the failure is
        // reported as a defect rather than hidden.
        if let Err(missing) = key_diff_check(&original, &fix.new_content, &fix.declared_removals) {
            refused.extend(missing);
            break;
        }

        patches.push(Patch {
            id: format!("fix-{}", round + 1),
            unified_diff: unified_diff(&original, &fix.new_content, &file_path),
            explanation: fix.explanation,
            source: PatchSource::Deterministic,
            confidence: fix.confidence,
            comments_preserved: fix.comments_preserved,
            declared_removals: fix.declared_removals,
            category: validation.category.clone(),
            new_content: fix.new_content.clone(),
        });
        current = fix.new_content;
    }

    if resolved {
        remaining_error.clear();
    }

    Ok(AutofixProposal {
        patches,
        resolved,
        remaining_error,
        refused,
    })
}

/// A temporary compose file living next to the original, removed on drop.
///
/// It must be a sibling: Compose resolves `build:` contexts and `env_file:`
/// paths relative to the file, so a copy in a temp directory validates a
/// different project than the one the user has.
struct ScratchFile {
    path: std::path::PathBuf,
}

impl ScratchFile {
    fn beside(file_path: &str) -> Result<Self, crate::error::ColimaError> {
        let original = std::path::Path::new(file_path);
        let dir = original.parent().ok_or_else(|| {
            crate::error::ColimaError::from("Compose file has no parent directory".to_string())
        })?;
        // The name must still end in .yml: the product's own path guard rejects
        // anything else, and a dotfile keeps it out of the user's way.
        let name = format!(
            ".colima-ui-autofix-{}.yml",
            std::process::id()
        );
        Ok(Self {
            path: dir.join(name),
        })
    }

    fn path_string(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    fn write(&self, contents: &str) -> Result<(), crate::error::ColimaError> {
        std::fs::write(&self.path, contents).map_err(|e| {
            crate::error::ColimaError::from(format!("Cannot write validation scratch file: {}", e))
        })
    }
}

impl Drop for ScratchFile {
    fn drop(&mut self) {
        // Best effort: leaving a stray dotfile is a nuisance, not a failure
        // worth propagating out of a destructor.
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WITH_SECRETS: &str = r#"
services:
  web:
    image: nginx
    environment:
      POSTGRES_PASSWORD: hunter2
    ports:
      - "80:80"
"#;

    #[test]
    fn accepts_a_patch_that_only_adds() {
        let new = format!("{}volumes:\n  dbdata:\n", WITH_SECRETS);
        assert!(key_diff_check(WITH_SECRETS, &new, &[]).is_ok());
    }

    #[test]
    fn blocks_a_patch_that_drops_the_environment_block() {
        // The failure the whole check exists for: this output validates fine.
        let new = r#"
services:
  web:
    image: nginx
    ports:
      - "80:80"
"#;
        let err = key_diff_check(WITH_SECRETS, new, &[]).unwrap_err();
        let paths: Vec<&str> = err.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.contains(&"services.web.environment"));
    }

    #[test]
    fn a_declared_removal_takes_its_subtree_with_it() {
        let new = r#"
services:
  web:
    image: nginx
    ports:
      - "80:80"
"#;
        let declared = vec!["services.web.environment".to_string()];
        assert!(key_diff_check(WITH_SECRETS, new, &declared).is_ok());
    }

    #[test]
    fn renaming_a_misspelled_key_is_allowed_when_declared() {
        let old = "services:\n  web:\n    imge: nginx\n";
        let new = "services:\n  web:\n    image: nginx\n";
        assert!(key_diff_check(old, new, &[]).is_err());
        let declared = vec!["services.web.imge".to_string()];
        assert!(key_diff_check(old, new, &declared).is_ok());
    }

    #[test]
    fn coercing_a_scalar_to_a_list_keeps_the_key() {
        let old = "services:\n  web:\n    ports: \"8080:80\"\n";
        let new = "services:\n  web:\n    ports:\n      - \"8080:80\"\n";
        assert!(key_diff_check(old, new, &[]).is_ok());
    }

    #[test]
    fn an_unparseable_original_is_not_a_reason_to_refuse() {
        // Tab-indented files do not parse; fixing them is the point.
        let old = "services:\n\tweb:\n\t\timage: nginx\n";
        let new = "services:\n  web:\n    image: nginx\n";
        assert!(key_diff_check(old, new, &[]).is_ok());
    }

    #[test]
    fn a_patch_producing_invalid_yaml_is_refused() {
        let new = "services:\n  web:\n   image: nginx\n  bad: [unclosed\n";
        assert!(key_diff_check(WITH_SECRETS, new, &[]).is_err());
    }

    /// The corpus run, against real `docker compose config`.
    ///
    /// Ignored by default because it needs Docker, which CI for this crate does
    /// not provide. It is the only test that proves the absolute criterion the
    /// fixability gate set: **no file is left worse than it was found**. Run it
    /// with `cargo test --lib -- --ignored corpus`.
    #[tokio::test]
    #[ignore = "requires docker"]
    async fn corpus_is_never_made_worse() {
        let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/compose-corpus");
        let work = std::env::temp_dir().join("colima-ui-autofix-corpus");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();

        let (mut fixed, mut total) = (0, 0);
        for entry in std::fs::read_dir(&corpus).unwrap() {
            let src = entry.unwrap().path();
            if src.extension().and_then(|e| e.to_str()) != Some("yml") {
                continue;
            }
            total += 1;
            // Copied out of the corpus so a fixer cannot mutate the fixtures.
            let target = work.join(src.file_name().unwrap());
            std::fs::copy(&src, &target).unwrap();
            let path = target.to_string_lossy().to_string();

            let proposal = compose_autofix_propose(path.clone()).await.unwrap();
            assert!(
                proposal.refused.is_empty(),
                "{:?}: a fixer removed keys it did not declare: {:?}",
                src.file_name().unwrap(),
                proposal.refused
            );

            if let Some(last) = proposal.patches.last() {
                // Every proposed patch must leave the file at least as valid as
                // it was. Writing it and re-validating is the only honest check.
                std::fs::write(&target, &last.new_content).unwrap();
                let after = crate::commands::compose_diagnose::compose_validate(path)
                    .await
                    .unwrap();
                assert!(
                    after.valid == proposal.resolved,
                    "{:?}: proposal.resolved disagrees with a fresh validation",
                    src.file_name().unwrap()
                );
                if after.valid {
                    fixed += 1;
                }
            }
        }
        let _ = std::fs::remove_dir_all(&work);
        assert!(total >= 15, "corpus shrank to {} files", total);
        // Deliberately not asserting a rate: the gate's verdict was ~35%, and
        // pinning a number here would turn a measurement into a target.
        println!("corpus: {}/{} fully resolved", fixed, total);
    }

    #[test]
    fn diff_marks_additions_and_removals() {
        let d = unified_diff("a\nb\n", "a\nc\n", "compose.yml");
        assert!(d.contains("-b"));
        assert!(d.contains("+c"));
        assert!(d.contains(" a"));
    }
}
