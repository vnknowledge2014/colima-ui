//! Turning a failing security rule into a concrete edit to the user's Dockerfile.
//!
//! Two things keep this honest.
//!
//! **The patch shape and the apply path are borrowed, not rebuilt.** A patch
//! here is a [`Patch`] with `source: Security`, applied through
//! `compose_autofix_apply`. One write path means one place where backup, undo
//! and the pre-write check live.
//!
//! **Almost nothing is auto-applicable, and the module says so.** A hardening
//! change that a tool cannot verify — adding a `USER` that may not exist in the
//! base image, pinning a tag to a version nobody chose, merging layers whose
//! order carries meaning — can turn a working image into one that does not
//! build. Those are produced as concrete diffs the user reviews and applies
//! deliberately, flagged `auto_applicable: false` with the risk stated. Exactly
//! one repair is mechanical enough to offer as a straight apply, and pretending
//! otherwise would be selling confidence rather than safety.

use super::compose_autofix::{unified_diff, Patch, PatchSource};
use super::dockerfile_parse::Dockerfile;
use serde::{Deserialize, Serialize};

/// A patch answering one failing rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPatch {
    /// The rule this answers. Always issued by the engine — a model is never
    /// allowed to invent a rule id, only to write prose about one.
    pub rule_id: String,
    pub patch: Patch,
    /// False when applying this could break a build that currently works.
    pub auto_applicable: bool,
    /// Why it cannot simply be applied. Present exactly when
    /// `auto_applicable` is false.
    pub risk_note: Option<String>,
}

/// An instruction that vanished from a patched Dockerfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingInstruction {
    pub keyword: String,
    pub argument: String,
}

/// The Dockerfile counterpart of `key_diff_check`.
///
/// YAML has keys; a Dockerfile has instructions, and the same failure mode
/// applies: a patch that drops the `COPY` carrying the application still
/// produces a file that builds, just not the image the user had. Every
/// instruction present before must be present after unless the patch declares
/// it is removing it.
pub fn instruction_diff_check(
    old_source: &str,
    new_source: &str,
    declared_removals: &[String],
) -> Result<(), Vec<MissingInstruction>> {
    let old = Dockerfile::parse(old_source);
    let new = Dockerfile::parse(new_source);

    let mut remaining: Vec<(String, String)> = new
        .instructions
        .iter()
        .map(|i| (i.keyword.clone(), i.argument.clone()))
        .collect();

    let mut missing = Vec::new();
    for ins in &old.instructions {
        // Multiset, not set: two identical `RUN` lines are two layers, and
        // losing one of them is a real change.
        if let Some(pos) = remaining
            .iter()
            .position(|(k, a)| *k == ins.keyword && *a == ins.argument)
        {
            remaining.remove(pos);
            continue;
        }
        // Declared as `KEYWORD argument`, or as the bare keyword to cover every
        // instance of it.
        let declared = declared_removals.iter().any(|d| {
            d == &format!("{} {}", ins.keyword, ins.argument) || d == &ins.keyword
        });
        if !declared {
            missing.push(MissingInstruction {
                keyword: ins.keyword.clone(),
                argument: ins.argument.clone(),
            });
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Rules this module can answer with a concrete diff.
///
/// Used by triage to label effort, so that "we can show you the exact edit" and
/// "you are on your own" are not presented as the same amount of work.
pub const ADDRESSABLE_RULES: &[&str] = &[
    "add-instead-of-copy",
    "runs-as-root",
    "no-healthcheck",
    "missing-source-label",
];

/// Rules whose patch is safe to apply without the user reading it.
pub const AUTO_APPLICABLE_RULES: &[&str] = &["add-instead-of-copy"];

/// Build patches for the failing rules, deterministically and offline.
///
/// Nothing here consults a model or the network. A rule with no mechanical
/// answer produces no patch rather than a guess.
pub fn propose_deterministic(
    dockerfile_source: &str,
    failing_rule_ids: &[String],
    path_label: &str,
) -> Vec<SecurityPatch> {
    let mut out = Vec::new();
    for rule_id in failing_rule_ids {
        let built = match rule_id.as_str() {
            "add-instead-of-copy" => fix_add_to_copy(dockerfile_source),
            "runs-as-root" => suggest_user(dockerfile_source),
            "no-healthcheck" => suggest_healthcheck(dockerfile_source),
            "missing-source-label" => suggest_source_label(dockerfile_source),
            _ => None,
        };
        let Some(built) = built else { continue };
        let Built {
            new_source,
            explanation,
            auto_applicable,
            risk_note,
            confidence,
            declared_removals,
        } = built;
        if new_source == dockerfile_source {
            continue;
        }
        // The gate, before the patch is ever returned. A security fix that
        // quietly deletes an instruction is the worst possible outcome here.
        //
        // Rewriting `ADD` to `COPY` genuinely removes the `ADD`, so a fixer
        // that changes an instruction must declare it — otherwise the check
        // rejects exactly the repair it exists to protect.
        if instruction_diff_check(dockerfile_source, &new_source, &declared_removals).is_err() {
            continue;
        }
        out.push(SecurityPatch {
            rule_id: rule_id.clone(),
            patch: Patch {
                id: format!("sec-{}", rule_id),
                unified_diff: unified_diff(dockerfile_source, &new_source, path_label),
                new_content: new_source,
                explanation,
                source: PatchSource::Security,
                confidence,
                // Editing the text keeps every comment the user wrote.
                comments_preserved: true,
                declared_removals,
                category: "security".to_string(),
            },
            auto_applicable,
            risk_note,
        });
    }
    out
}

/// What a fixer produces before it becomes a [`SecurityPatch`].
struct Built {
    new_source: String,
    explanation: String,
    auto_applicable: bool,
    risk_note: Option<String>,
    confidence: f32,
    /// Instructions this fixer removes on purpose, as `KEYWORD argument`.
    declared_removals: Vec<String>,
}

/// `ADD` a local path is `COPY` with extra, mostly unwanted, behaviour.
///
/// Skipped for remote URLs and for archives: `ADD` unpacks a tarball and
/// downloads a URL, so rewriting those to `COPY` changes what the image
/// contains. That distinction is what makes the remainder safe to apply
/// without review.
fn fix_add_to_copy(source: &str) -> Option<Built> {
    let df = Dockerfile::parse(source);
    let mut edited = df.clone();
    let mut declared_removals = Vec::new();

    let targets: Vec<(usize, String)> = df
        .instructions
        .iter()
        .filter(|ins| ins.keyword == "ADD")
        .filter(|ins| {
            let first = ins.argument.split_whitespace().next().unwrap_or("");
            let is_remote = first.starts_with("http://")
                || first.starts_with("https://")
                || first.starts_with("git@");
            let is_archive = [".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tar.xz", ".gz", ".bz2", ".xz", ".zip"]
                .iter()
                .any(|ext| first.ends_with(ext));
            !is_remote && !is_archive
        })
        .map(|ins| (ins.start_line, format!("{} {}", ins.keyword, ins.argument)))
        .collect();

    for (line_index, removed) in targets {
        let Some(line) = df.line(line_index) else {
            continue;
        };
        // Replace only the keyword, preserving indentation and the arguments
        // exactly as written.
        let Some(pos) = line.to_uppercase().find("ADD") else {
            continue;
        };
        let replacement = format!("{}COPY{}", &line[..pos], &line[pos + 3..]);
        edited.replace_line(line_index, &replacement);
        declared_removals.push(removed);
    }

    if declared_removals.is_empty() {
        return None;
    }
    Some(Built {
        new_source: edited.to_source(),
        explanation: format!(
            "Replaced {} local `ADD` with `COPY`. `ADD` also unpacks archives and fetches URLs, so `COPY` is the narrower instruction for a plain file copy.",
            declared_removals.len()
        ),
        auto_applicable: true,
        risk_note: None,
        confidence: 0.95,
        declared_removals,
    })
}

/// Propose a non-root `USER` at the end of the final stage.
///
/// Never auto-applied: the account has to exist in the base image, and this
/// cannot know whether it does. Shown as a diff so the decision is informed
/// rather than described.
fn suggest_user(source: &str) -> Option<Built> {
    let df = Dockerfile::parse(source);
    if df.instructions.iter().any(|i| i.keyword == "USER") {
        // A `USER` exists and the rule still failed, so the account it names is
        // root. Which account should replace it is the author's call.
        return None;
    }
    let anchor = df.last_stage_end()?;
    let mut edited = df.clone();
    edited.insert_after(
        anchor,
        &[
            "# Added by Colima UI: run as a non-root account.".to_string(),
            "# Replace `appuser` with an account that exists in this base image,".to_string(),
            "# or create one first (Alpine: `adduser -D appuser`; Debian: `useradd -m appuser`).".to_string(),
            "USER appuser".to_string(),
        ],
    );
    Some(Built {
        new_source: edited.to_source(),
        explanation: "Run the container as a non-root account instead of root.".to_string(),
        auto_applicable: false,
        risk_note: Some(
            "`appuser` must exist in the base image. Applied unchanged against an image that does not have it, the container will fail to start."
                .to_string(),
        ),
        confidence: 0.5,
        declared_removals: vec![],
    })
}

/// Propose a `HEALTHCHECK` built around a port the file already exposes.
fn suggest_healthcheck(source: &str) -> Option<Built> {
    let df = Dockerfile::parse(source);
    if df.instructions.iter().any(|i| i.keyword == "HEALTHCHECK") {
        return None;
    }
    let port = df
        .instructions
        .iter()
        .filter(|i| i.keyword == "EXPOSE")
        .filter_map(|i| i.argument.split_whitespace().next())
        .filter_map(|p| p.split('/').next())
        .find_map(|p| p.parse::<u32>().ok())?;

    let anchor = df.last_stage_end()?;
    let mut edited = df.clone();
    edited.insert_after(
        anchor,
        &[
            "# Added by Colima UI: adjust the path to a real health endpoint.".to_string(),
            format!(
                "HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://localhost:{}/ || exit 1",
                port
            ),
        ],
    );
    Some(Built {
        new_source: edited.to_source(),
        explanation: format!("Add a health check against the exposed port {}.", port),
        auto_applicable: false,
        risk_note: Some(
            "Assumes the port speaks HTTP at `/` and that `wget` exists in the image. Both are worth checking before applying."
                .to_string(),
        ),
        confidence: 0.4,
        declared_removals: vec![],
    })
}

/// Propose the OCI source label, with the URL left for the author to fill in.
fn suggest_source_label(source: &str) -> Option<Built> {
    let df = Dockerfile::parse(source);
    if df
        .instructions
        .iter()
        .any(|i| i.keyword == "LABEL" && i.argument.contains("org.opencontainers.image.source"))
    {
        return None;
    }
    let anchor = df.last_stage_end()?;
    let mut edited = df.clone();
    edited.insert_after(
        anchor,
        &[
            "# Added by Colima UI: point this at the repository that builds the image.".to_string(),
            "LABEL org.opencontainers.image.source=\"https://example.com/your/repo\"".to_string(),
        ],
    );
    Some(Built {
        new_source: edited.to_source(),
        explanation:
            "Declare where the image is built from, so a consumer can trace it back to source."
                .to_string(),
        auto_applicable: false,
        risk_note: Some(
            "The URL is a placeholder and must be replaced before applying.".to_string(),
        ),
        confidence: 0.3,
        declared_removals: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_add_becomes_copy_and_nothing_else_moves() {
        let src = "# app\nFROM alpine:3.19\nADD ./app /srv/app\nCMD [\"/srv/app\"]\n";
        let patches = propose_deterministic(src, &["add-instead-of-copy".into()], "Dockerfile");
        assert_eq!(patches.len(), 1);
        assert!(patches[0].auto_applicable);
        assert_eq!(
            patches[0].patch.new_content,
            "# app\nFROM alpine:3.19\nCOPY ./app /srv/app\nCMD [\"/srv/app\"]\n"
        );
    }

    #[test]
    fn a_url_or_archive_add_is_left_alone() {
        // Rewriting these to COPY would stop the download and stop the unpack.
        let src = "FROM alpine:3.19\nADD https://example.com/t.sh /t\nADD release.tar.gz /opt/\n";
        assert!(propose_deterministic(src, &["add-instead-of-copy".into()], "Dockerfile").is_empty());
    }

    #[test]
    fn a_user_suggestion_is_never_auto_applicable() {
        let src = "FROM alpine:3.19\nRUN true\n";
        let patches = propose_deterministic(src, &["runs-as-root".into()], "Dockerfile");
        assert_eq!(patches.len(), 1);
        assert!(!patches[0].auto_applicable);
        assert!(patches[0].risk_note.is_some());
        assert!(patches[0].patch.new_content.contains("USER appuser"));
    }

    #[test]
    fn user_lands_in_the_final_stage_not_the_builder() {
        let src = "FROM golang AS build\nRUN go build\n\nFROM alpine\nCOPY --from=build /app /app\n";
        let patches = propose_deterministic(src, &["runs-as-root".into()], "Dockerfile");
        let out = &patches[0].patch.new_content;
        let user_at = out.find("USER appuser").unwrap();
        let copy_at = out.find("COPY --from=build").unwrap();
        assert!(user_at > copy_at, "USER must come after the final stage's work");
    }

    #[test]
    fn a_healthcheck_needs_a_port_to_check() {
        let no_port = "FROM alpine:3.19\nCMD [\"sh\"]\n";
        assert!(propose_deterministic(no_port, &["no-healthcheck".into()], "Dockerfile").is_empty());

        let with_port = "FROM nginx\nEXPOSE 8080/tcp\n";
        let patches = propose_deterministic(with_port, &["no-healthcheck".into()], "Dockerfile");
        assert!(patches[0].patch.new_content.contains("localhost:8080"));
    }

    #[test]
    fn a_rule_with_no_mechanical_answer_produces_no_patch() {
        let src = "FROM ubuntu:latest\n";
        assert!(propose_deterministic(src, &["mutable-tag".into()], "Dockerfile").is_empty());
    }

    #[test]
    fn dropping_an_instruction_is_caught() {
        let old = "FROM alpine\nCOPY . /app\nCMD [\"/app\"]\n";
        let new = "FROM alpine\nCMD [\"/app\"]\n";
        let missing = instruction_diff_check(old, new, &[]).unwrap_err();
        assert_eq!(missing[0].keyword, "COPY");
        // Declaring it makes the same patch acceptable.
        assert!(instruction_diff_check(old, new, &["COPY . /app".to_string()]).is_ok());
    }

    #[test]
    fn two_identical_run_lines_are_two_layers() {
        let old = "FROM alpine\nRUN echo a\nRUN echo a\n";
        let new = "FROM alpine\nRUN echo a\n";
        assert!(instruction_diff_check(old, new, &[]).is_err());
    }

    #[test]
    fn adding_instructions_is_always_fine() {
        let old = "FROM alpine\n";
        let new = "FROM alpine\nUSER appuser\n";
        assert!(instruction_diff_check(old, new, &[]).is_ok());
    }
}
