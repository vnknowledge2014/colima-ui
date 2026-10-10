//! A guard that every action worth logging actually logs.
//!
//! Recording is wired in by hand at roughly thirty call sites, which makes
//! forgetting one the most likely defect in the whole feature — and the
//! quietest. A missing `record` breaks nothing, fails no test, and shows up
//! only as a gap in a history nobody can tell is incomplete. Reviewing thirty
//! diffs catches it once; this catches it every time.
//!
//! # How it works, and what that costs
//!
//! The command sources are pulled in with `include_str!` and scanned for a
//! recording call inside each named function. That is a text search, not a
//! semantic one, so it is blunt in both directions: renaming
//! `activity::record` or reformatting a call across lines will break it even
//! though the code is correct.
//!
//! That trade was made deliberately. This test fails **loudly**, at build time,
//! in a file that says what to do about it — whereas the mistake it guards
//! against fails **silently**, months later, when somebody goes looking for the
//! prune that deleted their images and finds nothing. A noisy false alarm is
//! cheap; a silent gap in an audit log is not.
//!
//! # Adding a command
//!
//! Add it to [`MUTATING_COMMANDS`]. A command that changes the machine and is
//! not in this list is invisible to the guard, so the list is the contract.

#![cfg(test)]

const CONTAINERS: &str = include_str!("containers.rs");
const VOLUMES: &str = include_str!("volumes.rs");
const NETWORKS: &str = include_str!("networks.rs");
const COLIMA: &str = include_str!("colima.rs");
const FILE_TRANSFER: &str = include_str!("file_transfer.rs");
const COLIMA_CONFIG: &str = include_str!("colima_config.rs");
const SELF_HEAL: &str = include_str!("self_heal.rs");

/// Every command that changes the machine, and the source it lives in.
///
/// Grouped the way the activity log groups them, so a reader can see at a
/// glance whether a whole category was forgotten rather than one command.
const MUTATING_COMMANDS: &[(&str, &str)] = &[
    // Destructive — cannot be undone.
    ("remove_container", CONTAINERS),
    ("remove_image", CONTAINERS),
    ("prune_images", CONTAINERS),
    ("system_prune", CONTAINERS),
    ("remove_volume", VOLUMES),
    ("prune_volumes", VOLUMES),
    ("remove_network", NETWORKS),
    ("prune_networks", NETWORKS),
    ("delete_instance", COLIMA),
    // Lifecycle — what was running, and when.
    ("start_container", CONTAINERS),
    ("stop_container", CONTAINERS),
    ("restart_container", CONTAINERS),
    ("pause_container", CONTAINERS),
    ("unpause_container", CONTAINERS),
    ("run_container", CONTAINERS),
    ("rename_container", CONTAINERS),
    ("start_instance", COLIMA),
    ("stop_instance", COLIMA),
    // Tasks — took time, and either finished or did not.
    ("pull_image", CONTAINERS),
    ("start_image_save", FILE_TRANSFER),
    ("start_image_load", FILE_TRANSFER),
    ("start_copy_to_container", FILE_TRANSFER),
    ("start_copy_from_container", FILE_TRANSFER),
    // Config — why the machine behaves differently today.
    ("apply_colima_config", COLIMA_CONFIG),
];

/// The text of one function, from its signature to the closing brace in column
/// zero.
///
/// Relies on the repo's formatting — top-level items close at column zero —
/// which `cargo fmt` guarantees and which no function here departs from.
fn body_of(source: &str, name: &str) -> Option<String> {
    let needle = format!("fn {}(", name);
    let start = source.find(&needle)?;
    let rest = &source[start..];
    let end = rest.find("\n}").map_or(rest.len(), |i| i + 2);
    Some(rest[..end].to_string())
}

/// True when this function records, directly or through the one helper that
/// records on its behalf.
///
/// The four background transfers hand off to `spawn_job`, which writes the row
/// when the job ends rather than when it starts — so their own bodies contain
/// no `record` call and never should. `spawn_job` itself is checked separately.
fn records_activity(body: &str) -> bool {
    body.contains("activity::record(") || body.contains("spawn_job(")
}

#[test]
fn every_mutating_command_records_what_it_did() {
    let mut missing = Vec::new();
    for (name, source) in MUTATING_COMMANDS {
        match body_of(source, name) {
            Some(body) if records_activity(&body) => {}
            Some(_) => missing.push(format!("{name}: no activity::record in its body")),
            // A renamed or deleted command is also a finding: the list is the
            // contract, and silently skipping an entry defeats the guard.
            None => missing.push(format!("{name}: not found — renamed or removed?")),
        }
    }
    assert!(
        missing.is_empty(),
        "these actions change the machine but leave no trace:\n  {}\n\n\
         Add `activity::record(...)` to each, or update MUTATING_COMMANDS if a \
         command was renamed.",
        missing.join("\n  ")
    );
}

/// The handoff the four transfers depend on.
///
/// Without this, `records_activity` accepting `spawn_job(` would be a hole big
/// enough to drive every background job through.
#[test]
fn spawn_job_records_on_behalf_of_the_transfers() {
    let body = body_of(FILE_TRANSFER, "spawn_job").expect("spawn_job must exist");
    assert!(
        body.contains("subject.record("),
        "spawn_job is what makes the four transfers count as recorded; \
         without a record call there, they are all silent"
    );
}

/// Every terminal branch of a transfer writes a row, including the ones that
/// are not failures.
#[test]
fn a_transfer_records_each_way_it_can_end() {
    let body = body_of(FILE_TRANSFER, "spawn_job").expect("spawn_job must exist");
    let calls = body.matches("subject.record(").count();
    assert!(
        calls >= 4,
        "spawn_job ends four ways — cancelled early, cancelled part-way, \
         succeeded, failed — and each is worth a row; found {calls}"
    );
    for outcome in ["Cancelled", "Ok", "Failed"] {
        assert!(
            body.contains(&format!("ActivityOutcome::{outcome}")),
            "no branch records {outcome}"
        );
    }
}

/// Both writers of a self-heal must spell the action the same way.
///
/// The timeline folds `heal_log` and `activity_log` into one row by comparing
/// their verbs. `from_heal` is pinned to `HealAction::as_str` by a unit test,
/// but the other half — what `self_heal` writes into `activity_log` — lives in
/// a database call no unit test reaches. A hand-written literal there, or a
/// `format!("{:?}", action)`, would silently bring the duplicate rows back.
///
/// Checked by reading the source for the same reason as the guard above: the
/// failure is invisible at runtime.
#[test]
fn self_heal_records_its_action_using_the_shared_spelling() {
    let body = body_of(SELF_HEAL, "perform").expect("perform must exist");
    assert!(
        body.contains("action.as_str()"),
        "self_heal must write the activity_log verb with `HealAction::as_str`; \
         any other spelling stops the timeline folding the two rows for one heal"
    );
    assert!(
        !body.contains("format!(\"{:?}\", action)"),
        "a Debug rendering drops the underscore and matches nothing"
    );
}

/// The guard is only as good as its list.
#[test]
fn the_list_covers_every_group() {
    assert!(
        MUTATING_COMMANDS.len() >= 24,
        "the list shrank — a command was removed from the guard rather than \
         from the app?"
    );
}
