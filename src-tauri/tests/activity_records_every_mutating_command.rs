//! Every command that changes the machine must write it down.
//!
//! ## Why this is a source scan and not a real test
//!
//! Forgetting to record is silent. The command still works, the user sees
//! nothing wrong, and the gap only shows up months later when somebody asks
//! what deleted their volume and the answer is missing. Nothing at runtime can
//! notice an absence like that, so this reads the source instead.
//!
//! It is crude, and it breaks when a function is renamed or reformatted. That
//! trade was made deliberately: **it breaks loudly, at CI, instead of silently,
//! in production.** A renamed function costs one line in the table below; a
//! forgotten `record` costs an audit trail nobody knew they had lost.
//!
//! ## Keeping it honest
//!
//! The scan looks for `activity::record` *between* one command's `pub async fn`
//! and the next item in the file, so a `record` in a neighbouring function does
//! not count as covering this one. `covers_only_the_function_it_is_in` proves
//! that: it is the test that fails if this file ever starts passing for the
//! wrong reason.

/// The commands that change something, and the file each one lives in.
///
/// This list is data on purpose. Remembering to add a command here is easier
/// than remembering to instrument it, and forgetting to add it is the one
/// mistake this file cannot catch — which is why the list is grouped and
/// commented rather than sorted by accident.
const MUTATING: &[(&str, &str)] = &[
    // Destructive — cannot be undone, and the whole reason this store exists.
    ("src/commands/containers.rs", "remove_container"),
    ("src/commands/containers.rs", "remove_image"),
    ("src/commands/containers.rs", "prune_images"),
    ("src/commands/containers.rs", "system_prune"),
    ("src/commands/volumes.rs", "remove_volume"),
    ("src/commands/volumes.rs", "prune_volumes"),
    ("src/commands/networks.rs", "remove_network"),
    ("src/commands/networks.rs", "prune_networks"),
    ("src/commands/colima.rs", "delete_instance"),
    // Lifecycle — on its own it explains "why was it down last night", which is
    // the question metrics history cannot answer by itself.
    ("src/commands/containers.rs", "start_container"),
    ("src/commands/containers.rs", "stop_container"),
    ("src/commands/containers.rs", "restart_container"),
    ("src/commands/containers.rs", "pause_container"),
    ("src/commands/containers.rs", "unpause_container"),
    ("src/commands/containers.rs", "run_container"),
    ("src/commands/containers.rs", "rename_container"),
    ("src/commands/colima.rs", "start_instance"),
    ("src/commands/colima.rs", "stop_instance"),
    // Task — took time, and either finished or did not.
    ("src/commands/containers.rs", "pull_image"),
    // Config — why today behaves differently from yesterday.
    ("src/commands/colima_config.rs", "apply_colima_config"),
    ("src/commands/alerts.rs", "alerts_save_rule"),
    ("src/commands/alerts.rs", "alerts_delete_rule"),
    ("src/commands/security_policy.rs", "security_policy_save"),
    ("src/commands/security_policy.rs", "security_policy_delete"),
    ("src/commands/security_watch.rs", "security_watch_set_enabled"),
    ("src/commands/security_watch.rs", "security_watch_set_interval"),
    ("src/commands/self_heal.rs", "self_heal_save_rule"),
    ("src/commands/self_heal.rs", "self_heal_set_enabled"),
];

/// Background transfers record from one shared place instead of four.
///
/// `image_save`, `image_load` and the two `copy_*` commands all return a job id
/// and finish later, so recording at the command would log a beginning with no
/// end. They settle through `transfer_registry::settle`, which is where the
/// outcome and the duration actually exist — so that is what this checks.
const SETTLES_TRANSFERS: (&str, &str) = ("src/transfer_registry.rs", "settle");

/// The body of `pub async fn <name>`, up to the next top-level item.
fn body_of<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let start = source.find(&format!("pub async fn {name}("))?;
    let rest = &source[start..];
    // The next `pub ` at the start of a line is the next item. Commands are
    // top-level, so this does not cut a body short at a nested definition.
    let end = rest[1..].find("\npub ").map_or(rest.len(), |i| i + 1);
    Some(&rest[..end])
}

fn source_of(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()))
}

#[test]
fn every_mutating_command_records_what_it_did() {
    let mut missing = Vec::new();

    for (path, name) in MUTATING {
        let source = source_of(path);
        let Some(body) = body_of(&source, name) else {
            panic!(
                "{path} has no `pub async fn {name}` — either it was renamed, in \
                 which case fix the table in this file, or it was deleted, in \
                 which case remove the row"
            );
        };
        if !body.contains("activity::record") {
            missing.push(format!("{path}::{name}"));
        }
    }

    assert!(
        missing.is_empty(),
        "these commands change the machine without writing it down:\n  {}\n\
         Add `activity::record(...)` to each, or remove it from the table in \
         tests/activity_records_every_mutating_command.rs if it no longer mutates.",
        missing.join("\n  ")
    );
}

#[test]
fn background_transfers_record_where_they_finish() {
    let (path, name) = SETTLES_TRANSFERS;
    let source = source_of(path);
    let body = source
        .find(&format!("pub fn {name}("))
        .map(|start| &source[start..])
        .expect("transfer_registry::settle exists");

    assert!(
        body.contains("activity::record"),
        "{path}::{name} is where every transfer reaches a real outcome. Without \
         a record here, exporting an image leaves no trace at all — or worse, a \
         trace saying it started."
    );
}

#[test]
fn covers_only_the_function_it_is_in() {
    // The guard above is only worth having if a `record` in the next function
    // cannot satisfy this one. Proven against a fake source rather than the
    // real tree, so the proof does not move when the tree does.
    let fake = "\
pub async fn instrumented() {
    activity::record(entry);
}

pub async fn forgotten() {
    do_something();
}
";
    let instrumented = body_of(fake, "instrumented").expect("found");
    let forgotten = body_of(fake, "forgotten").expect("found");

    assert!(instrumented.contains("activity::record"));
    assert!(
        !forgotten.contains("activity::record"),
        "a neighbour's record leaked into this body, so the guard would pass \
         for a command that records nothing"
    );
}
