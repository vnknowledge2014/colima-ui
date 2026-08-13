//! One timeline, read from two stores.
//!
//! Each store was built for its own question — "what did the user do to this
//! machine", "what did self-healing do" — and each answers it well. Neither
//! answers *what happened to this machine, in order*, because that answer is
//! spread across both.
//!
//! This module does not add a third store. It reads the two, normalises them to
//! one shape, and merges. Writing a combined table instead would mean every
//! action is recorded twice and the two copies can disagree; a read-time merge
//! cannot drift from its own sources.
//!
//! # A failing source must be visible
//!
//! If one store is locked, the timeline is still built from the other — and
//! says so. A timeline that silently drops a source reads as "nothing
//! happened", which is the one thing it must never say when something did.
//!
//! # Why anything needs de-duplicating
//!
//! Self-healing restarting a container writes two rows on purpose:
//! `heal_log` records that a rule ran, `activity_log` records that a container
//! restarted with `actor = app`. Both are correct and both are wanted — the
//! first explains the rule, the second belongs in the machine's history. But
//! they are one event, and a timeline showing it twice is wrong.

use serde::{Deserialize, Serialize};

use super::activity::{ActivityActor, ActivityFilter, ActivityKind, ActivityOutcome};
use super::self_heal::HealOutcome;

/// Where a row came from. Kept on the item so the UI can explain a row and so
/// a reader can tell a merged row from a plain one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedSource {
    Activity,
    SelfHeal,
    Alert,
    SecurityScan,
    ComposeFix,
}

/// One thing that happened, in the terms every source can be expressed in.
///
/// Deliberately not the union of all five row types: a shape that carries
/// every field of every source is one nobody can render without knowing which
/// source it came from, which defeats merging them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    /// Stable within one response, so a list can key on it.
    pub id: String,
    pub ts: i64,
    pub source: FeedSource,
    pub kind: ActivityKind,
    pub verb: String,
    pub target_kind: String,
    pub target: String,
    pub target_name: String,
    pub actor: ActivityActor,
    pub outcome: ActivityOutcome,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// Sources folded into this row, when it stands for more than one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub merged_from: Vec<FeedSource>,
}

/// The answer, plus what could not be read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feed {
    pub items: Vec<FeedItem>,
    /// Sources that failed to read. Non-empty means the timeline is
    /// incomplete, and the UI must say so rather than imply quiet.
    pub partial: Vec<String>,
}

/// How far apart two records of the same event can be and still be one event.
///
/// Self-healing writes its `heal_log` row and the restart's `activity_log` row
/// microseconds apart in the same call, so this only needs to absorb clock
/// granularity — not to guess. Two seconds is generous for that and still far
/// below the interval at which a person restarts the same container twice.
///
/// A guess all the same, and it should be re-measured against real data before
/// anyone widens it.
const MERGE_WINDOW_MS: i64 = 2_000;

/// Fold rows that describe the same event into one.
///
/// Pure, and separated from the reading, because this is the only part that can
/// be wrong in a way that loses information: too wide a window hides a real
/// second event, too narrow shows one event twice.
///
/// Input must be sorted newest-first; output preserves that order.
pub fn dedupe(items: Vec<FeedItem>) -> Vec<FeedItem> {
    let mut out: Vec<FeedItem> = Vec::with_capacity(items.len());

    for item in items {
        let twin = out.iter_mut().find(|kept| {
            // Same subject, same action, close in time — and one of the two is
            // the app acting on its own. A pair of user actions on the same
            // container is two clicks, not one event.
            (kept.ts - item.ts).abs() <= MERGE_WINDOW_MS
                && kept.target == item.target
                && kept.verb == item.verb
                && kept.source != item.source
                // **Both** sides must be the app. Self-healing writes both of
                // its rows as `App`, so nothing is lost by requiring it — and
                // requiring only one side would let a user's own restart,
                // clicked within two seconds of an automatic one, be folded
                // away. That deletes a real event rather than duplicating one.
                && kept.actor == ActivityActor::App
                && item.actor == ActivityActor::App
        });

        match twin {
            Some(kept) => {
                // Keep the richer row. `heal_log` knows which rule fired;
                // `activity_log` knows only that something restarted.
                kept.merged_from.push(item.source);
                if kept.detail.len() < item.detail.len() {
                    kept.detail = item.detail;
                }
            }
            None => out.push(item),
        }
    }
    out
}

/// Read every source, merge, and cut to the requested size.
///
/// A source that fails is named in `partial` rather than failing the call: four
/// fifths of a timeline is worth more than an error page, as long as the
/// missing fifth is admitted.
pub fn feed(filter: &ActivityFilter) -> Feed {
    let limit = filter.limit.unwrap_or(200).clamp(1, 1000);
    let mut items: Vec<FeedItem> = Vec::new();
    let mut partial: Vec<String> = Vec::new();

    // Four of the five stores cannot express `kind` or `target` as a query, so
    // their rows are filtered after they arrive. That makes the read size and
    // the answer size different questions: asking for 200 heal rows and then
    // keeping only one container's would report "nothing" for a container whose
    // rows sit just past row 200, while they plainly exist.
    //
    // So when a filter is set, read each store's maximum and let the filter
    // decide what survives. The stores cap themselves at 500, which is cheap to
    // read and far more than any filtered view needs.
    let filtered = filter.kind.is_some() || filter.target.is_some();
    let read_limit = if filtered { 500 } else { limit };

    match super::activity::query(filter) {
        Ok(rows) => items.extend(rows.into_iter().map(from_activity)),
        Err(e) => partial.push(format!("activity log: {e}")),
    }

    match super::self_heal::recent_log(read_limit) {
        Ok(rows) => items.extend(rows.into_iter().map(from_heal)),
        Err(e) => partial.push(format!("self-healing log: {e}")),
    }

    // Every filter applies to every source, not only to the one that can
    // express it as SQL. `activity::query` already narrowed its own rows; the
    // self-healing log cannot, so the same conditions are re-applied here.
    // Without this, choosing "Lifecycle" still returned every heal entry — a
    // filter that visibly does nothing to half the list.
    if let Some(kind) = filter.kind.as_deref() {
        // `as_str`, not `Debug`: the two agree for today's single-word variants
        // but would diverge the moment one has two words — which is exactly how
        // the heal verbs came to disagree.
        items.retain(|i| i.kind.as_str() == kind);
    }
    if let Some(target) = filter.target.as_deref() {
        items.retain(|i| i.target == target);
    }
    if let Some(from) = filter.from_ms {
        items.retain(|i| i.ts >= from);
    }
    if let Some(to) = filter.to_ms {
        items.retain(|i| i.ts <= to);
    }

    items.sort_by(|a, b| b.ts.cmp(&a.ts));
    let mut items = dedupe(items);
    items.truncate(limit as usize);

    Feed { items, partial }
}

fn from_activity(r: super::activity::ActivityRow) -> FeedItem {
    FeedItem {
        id: format!("activity-{}", r.id),
        ts: r.ts,
        source: FeedSource::Activity,
        kind: r.kind,
        verb: r.verb,
        target_kind: r.target_kind,
        target: r.target,
        target_name: r.target_name,
        actor: r.actor,
        outcome: r.outcome,
        detail: r.detail,
        duration_ms: r.duration_ms,
        merged_from: vec![],
    }
}

fn from_heal(r: super::self_heal::HealLogEntry) -> FeedItem {
    FeedItem {
        id: format!("heal-{}", r.id),
        ts: r.ts,
        source: FeedSource::SelfHeal,
        // Self-healing restarts things; that is a lifecycle change whoever
        // decided on it.
        kind: ActivityKind::Lifecycle,
        // The same string `self_heal` writes into `activity_log` for this
        // event. Not a `Debug` rendering of the enum: that spells
        // `RestartContainer` as `restartcontainer`, which matches nothing, and
        // the two rows for one heal would never be folded together.
        verb: r.action.as_str().to_string(),
        target_kind: "container".to_string(),
        target: r.container_id,
        target_name: r.container_name,
        actor: ActivityActor::App,
        // Exhaustive, so a new `HealOutcome` variant fails to compile here
        // rather than landing quietly in `Ok`. Three of these mean the app
        // *did not act*; reporting them as successes would have the timeline
        // claim a restart that never happened.
        outcome: match r.outcome {
            HealOutcome::Executed => ActivityOutcome::Ok,
            HealOutcome::Failed => ActivityOutcome::Failed,
            // Advisory rules never act by design. `Denied` is the only word
            // here for "the machine was not changed", which is the fact a
            // reader needs; the rule's own text in `detail` says it was advice
            // rather than a refusal.
            HealOutcome::Suggested => ActivityOutcome::Denied,
            HealOutcome::QuotaBlocked
            | HealOutcome::SwitchedOff => ActivityOutcome::Denied,
        },
        detail: format!("{}: {}", r.rule_name, r.detail),
        duration_ms: None,
        merged_from: vec![],
    }
}

/// Render the timeline as CSV.
///
/// One row per event, in the order shown. Quoted the plain way — doubling an
/// embedded quote — because a detail line can contain a comma, a quote or a
/// newline, and a log that breaks the spreadsheet it was exported for is not
/// an export.
pub fn to_csv(items: &[FeedItem]) -> String {
    fn field(value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }

    let mut out = String::from(
        "timestamp,source,kind,verb,target_kind,target,target_name,actor,outcome,detail,duration_ms\n",
    );
    for i in items {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            i.ts,
            field(&format!("{:?}", i.source).to_lowercase()),
            field(i.kind.as_str()),
            field(&i.verb),
            field(&i.target_kind),
            field(&i.target),
            field(&i.target_name),
            field(&format!("{:?}", i.actor).to_lowercase()),
            field(&format!("{:?}", i.outcome).to_lowercase()),
            field(&i.detail),
            i.duration_ms.map(|d| d.to_string()).unwrap_or_default(),
        ));
    }
    out
}

/// Write the timeline to a file the user chose.
///
/// Confined to the picked folder with the same check the diagnostics export
/// uses: the picker records intent, it is not an authorization boundary.
#[tauri::command]
pub async fn activity_export(
    dest_dir: String,
    file_name: String,
    format: String,
    overwrite: Option<bool>,
) -> Result<String, crate::error::ColimaError> {
    crate::helpers::run_blocking(move || {
        if dest_dir.trim().is_empty() || file_name.trim().is_empty() {
            return Err("Choose a folder and a file name".to_string());
        }
        let base = std::path::Path::new(&dest_dir);
        if !base.is_dir() {
            return Err(format!("Destination folder does not exist: {}", dest_dir));
        }
        let target = base.join(&file_name);
        crate::validation::assert_path_within(base, &target)?;
        if target.exists() && !overwrite.unwrap_or(false) {
            return Err(format!(
                "{} already exists. Choose another name or confirm overwriting.",
                file_name
            ));
        }

        let feed = feed(&ActivityFilter {
            limit: Some(1000),
            ..Default::default()
        });
        let body = match format.as_str() {
            "csv" => to_csv(&feed.items),
            _ => serde_json::to_string_pretty(&feed.items)
                .map_err(|e| format!("Cannot encode history: {e}"))?,
        };
        std::fs::write(&target, body).map_err(|e| format!("Cannot write {}: {}", file_name, e))?;
        Ok(target.to_string_lossy().to_string())
    })
    .await
    .map_err(crate::error::ColimaError::validation)
}

#[tauri::command]
pub async fn activity_feed(
    kind: Option<String>,
    target: Option<String>,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    limit: Option<i64>,
) -> Result<Feed, crate::error::ColimaError> {
    let filter = ActivityFilter {
        kind,
        target,
        from_ms,
        to_ms,
        limit,
    };
    crate::helpers::run_blocking(move || Ok(feed(&filter)))
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, ts: i64, source: FeedSource, actor: ActivityActor, detail: &str) -> FeedItem {
        FeedItem {
            id: id.to_string(),
            ts,
            source,
            kind: ActivityKind::Lifecycle,
            verb: "restart".to_string(),
            target_kind: "container".to_string(),
            target: "abc123".to_string(),
            target_name: "web".to_string(),
            actor,
            outcome: ActivityOutcome::Ok,
            detail: detail.to_string(),
            duration_ms: None,
            merged_from: vec![],
        }
    }

    /// The rows the two stores really produce for one self-heal, built by the
    /// same code paths production uses.
    ///
    /// Hand-writing both sides is what let a defect through before: the test
    /// set `verb` to the same string on both, while `from_heal` was rendering
    /// the enum as `restartcontainer` and `self_heal` was writing
    /// `restart_container`. The predicate could never match in production and
    /// the test could never notice. Anything asserting on `dedupe` has to come
    /// through the normalisers.
    fn real_heal_pair(action: super::super::self_heal::HealAction) -> Vec<FeedItem> {
        use super::super::activity::ActivityRow;
        use super::super::self_heal::{HealLogEntry, HealMode};

        let heal = from_heal(HealLogEntry {
            id: 1,
            ts: 10_000,
            rule_id: 7,
            rule_name: "crash-loop".to_string(),
            container_id: "abc123".to_string(),
            container_name: "web".to_string(),
            action,
            mode: HealMode::Auto,
            outcome: HealOutcome::Executed,
            detail: "restarted after 5 crashes".to_string(),
        });

        // What `self_heal` writes alongside it: same verb via `as_str`, same
        // container, `actor = app`.
        let logged = from_activity(ActivityRow {
            id: 1,
            ts: 9_980,
            kind: ActivityKind::Lifecycle,
            verb: action.as_str().to_string(),
            target_kind: "container".to_string(),
            target: "abc123".to_string(),
            target_name: "web".to_string(),
            actor: ActivityActor::App,
            outcome: ActivityOutcome::Ok,
            detail: String::new(),
            duration_ms: Some(120),
        });

        vec![heal, logged]
    }

    #[test]
    fn self_healing_shows_as_one_row_not_two() {
        // The exact case the design creates on purpose: one restart, recorded
        // by the rule engine and by the action log — through the real
        // normalisers, so the verb strings are the ones production compares.
        let out = dedupe(real_heal_pair(super::super::self_heal::HealAction::RestartContainer));
        assert_eq!(out.len(), 1, "one restart must not appear twice");
        // The rule name is the half worth keeping.
        assert!(out[0].detail.contains("crash-loop"));
        assert_eq!(out[0].merged_from, vec![FeedSource::Activity]);
    }

    #[test]
    fn the_two_stores_spell_the_verb_the_same_way() {
        // The defect itself, asserted directly: if these ever diverge again,
        // this fails with the two spellings side by side rather than leaving a
        // silently duplicated timeline.
        let pair = real_heal_pair(super::super::self_heal::HealAction::StopContainer);
        assert_eq!(
            pair[0].verb, pair[1].verb,
            "heal_log and activity_log must name the same action identically"
        );
        assert_eq!(pair[0].verb, "stop_container");
    }

    #[test]
    fn a_heal_that_was_blocked_is_not_reported_as_done() {
        use super::super::self_heal::{HealAction, HealLogEntry, HealMode};

        // The quota and the kill switch both mean the app did nothing.
        // Rendering those as `Ok` had the timeline claim a restart that never
        // happened.
        for outcome in [
            HealOutcome::QuotaBlocked,
            HealOutcome::SwitchedOff,
            HealOutcome::Suggested,
        ] {
            let row = from_heal(HealLogEntry {
                id: 1,
                ts: 1,
                rule_id: 1,
                rule_name: "r".to_string(),
                container_id: "abc".to_string(),
                container_name: "web".to_string(),
                action: HealAction::RestartContainer,
                mode: HealMode::Auto,
                outcome,
                detail: "blocked".to_string(),
            });
            assert_ne!(
                row.outcome,
                ActivityOutcome::Ok,
                "{outcome:?} means nothing was done to the machine"
            );
        }
    }

    #[test]
    fn two_restarts_ten_minutes_apart_stay_two_rows() {
        let items = vec![
            item("heal-1", 610_000, FeedSource::SelfHeal, ActivityActor::App, "rule: restarted"),
            item("activity-1", 10_000, FeedSource::Activity, ActivityActor::User, "restarted"),
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn a_user_restart_is_never_folded_into_an_app_one_from_the_same_source() {
        // Same source means two separate records of two separate things.
        let items = vec![
            item("activity-2", 10_000, FeedSource::Activity, ActivityActor::App, "restarted"),
            item("activity-1", 9_990, FeedSource::Activity, ActivityActor::User, "restarted"),
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn two_user_actions_close_together_are_both_kept() {
        // Nothing here is the app acting on its own, so there is no duplicate
        // to fold — just somebody clicking twice.
        let items = vec![
            item("activity-2", 10_000, FeedSource::Activity, ActivityActor::User, "restarted"),
            item("compose-1", 9_990, FeedSource::ComposeFix, ActivityActor::User, "restarted"),
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn different_containers_never_merge() {
        let mut other = item("activity-1", 9_990, FeedSource::Activity, ActivityActor::App, "restarted");
        other.target = "def456".to_string();
        let items = vec![
            item("heal-1", 10_000, FeedSource::SelfHeal, ActivityActor::App, "rule: restarted"),
            other,
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn different_verbs_never_merge() {
        let mut stopped = item("activity-1", 9_990, FeedSource::Activity, ActivityActor::App, "stopped");
        stopped.verb = "stop".to_string();
        let items = vec![
            item("heal-1", 10_000, FeedSource::SelfHeal, ActivityActor::App, "rule: restarted"),
            stopped,
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn an_empty_timeline_survives_deduping() {
        assert!(dedupe(vec![]).is_empty());
    }

    #[test]
    fn csv_survives_a_detail_containing_commas_quotes_and_newlines() {
        // Exactly what a runtime error message looks like, and exactly what
        // breaks a naive export.
        let mut row = item("activity-1", 1_000, FeedSource::Activity, ActivityActor::User, "");
        row.detail = "failed: \"no such image\", try `docker pull`\nand retry".to_string();

        let csv = to_csv(&[row]);
        let body = csv.lines().nth(1).expect("one data row");
        // The embedded quote is doubled, so the field stays one field.
        assert!(body.contains("\"\"no such image\"\""));
        // Header plus a row whose newline lives inside a quoted field: three
        // physical lines, one record.
        assert_eq!(csv.lines().count(), 3);
    }

    /// The filter compares against the string the frontend sends, so the two
    /// spellings have to be the same one.
    ///
    /// Guarding the same mistake that broke `dedupe`: a `Debug` rendering and a
    /// serde `snake_case` name agree only while every variant is one word, and
    /// they disagree silently — an empty list rather than an error.
    #[test]
    fn the_kind_filter_speaks_the_frontends_spelling() {
        for (kind, wire) in [
            (ActivityKind::Destructive, "destructive"),
            (ActivityKind::Lifecycle, "lifecycle"),
            (ActivityKind::Task, "task"),
            (ActivityKind::Config, "config"),
        ] {
            assert_eq!(
                kind.as_str(),
                wire,
                "src/lib/api/activity.ts sends {wire:?}"
            );
            // And the value serde puts on the wire is the same string.
            let json = serde_json::to_string(&kind).expect("serialisable");
            assert_eq!(json, format!("\"{wire}\""));
        }
    }

    #[test]
    fn csv_header_matches_the_column_count() {
        let row = item("activity-1", 1, FeedSource::Activity, ActivityActor::User, "ok");
        let csv = to_csv(&[row]);
        let header_cols = csv.lines().next().unwrap().split(',').count();
        assert_eq!(header_cols, 11, "header drifted from the row builder");
    }
}
