//! Menu bar / system tray.
//!
//! Lets the user see instance state and start or stop instances without opening
//! the main window.
//!
//! # Two things worth knowing before changing this
//!
//! **State granularity.** `instance_reader` decides an instance is running by
//! the presence of `ha.sock`/`ha.pid`, so the only states it can report are
//! Running and Stopped. "Starting…" is therefore tracked here, in
//! [`PendingOps`], from operations this app itself initiated — the tray does
//! not pretend to observe a transition it has no source for.
//!
//! **Event source.** State comes from `poller.rs`, which emits
//! `instances-update` on a 5s tick. `sse.rs` has a second, independent
//! publisher for browser mode; subscribing to that one instead would leave the
//! tray silent in the desktop app.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

use crate::commands::colima::ColimaInstance;

const TRAY_ID: &str = "colima-ui-tray";

/// Settings key controlling whether the tray is shown. Absent means "show it".
const SHOW_TRAY_SETTING: &str = "colimaui_show_tray";

/// Aggregate shown by the icon. Ordered by precedence: an operation in flight
/// outranks whatever the last poll said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Pending,
    Running,
    Stopped,
}

/// Profiles with an operation in flight, so their menu entries can be disabled.
///
/// Without this, a menu built 5 seconds ago still offers "Stop" for something
/// already stopping, and a second click fires a second command.
static PENDING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Last rendered snapshot, so the menu is only rebuilt when it would differ.
/// Rebuilding on every 5s tick makes the menu flicker while open.
type LastSnapshot = LazyLock<Mutex<Option<Vec<(String, String)>>>>;
static LAST_SNAPSHOT: LastSnapshot = LazyLock::new(|| Mutex::new(None));

fn snapshot_of(instances: &[ColimaInstance]) -> Vec<(String, String)> {
    instances
        .iter()
        .map(|i| (i.name.clone(), i.status.clone()))
        .collect()
}

pub fn aggregate_state(instances: &[ColimaInstance], pending: &HashSet<String>) -> TrayState {
    if !pending.is_empty() {
        return TrayState::Pending;
    }
    if instances
        .iter()
        .any(|i| i.status.eq_ignore_ascii_case("running"))
    {
        TrayState::Running
    } else {
        TrayState::Stopped
    }
}

/// Menu item ids encode an action and a profile: `instance:start:dev`.
///
/// Ids come back from the OS, so they are treated as untrusted input — see
/// [`parse_menu_id`].
#[derive(Debug, PartialEq, Eq)]
pub enum MenuAction {
    StartInstance(String),
    StopInstance(String),
    OpenContainer(String),
    ShowWindow,
    Help,
    /// Stop self-healing from here, without opening the window.
    ///
    /// The second of the two switches the feature is required to have. The
    /// first lives in Settings; this one works when the window is closed, and
    /// neither is behind the Pro gate.
    StopSelfHealing,
    Quit,
}

/// Parse a menu id, rejecting anything that is not a valid profile name.
///
/// A profile name reaches argv, so a value like `-rf` would be read by `colima`
/// as a flag. Validation happens here rather than at the command, because this
/// is the boundary where the untrusted value enters.
pub fn parse_menu_id(id: &str) -> Option<MenuAction> {
    match id {
        "show" => return Some(MenuAction::ShowWindow),
        "quit" => return Some(MenuAction::Quit),
        "help" => return Some(MenuAction::Help),
        "self-heal:stop" => return Some(MenuAction::StopSelfHealing),
        _ => {}
    }

    if let Some(rest) = id.strip_prefix("container:open:") {
        // Container names are user-controlled via docker; keep them out of
        // argv-like contexts — reject anything starting with a dash.
        if rest.is_empty() || rest.starts_with('-') {
            eprintln!("[tray] rejecting menu id with invalid container: {:?}", id);
            return None;
        }
        return Some(MenuAction::OpenContainer(rest.to_string()));
    }

    let rest = id.strip_prefix("instance:")?;
    let (verb, profile) = rest.split_once(':')?;

    if !crate::validation::is_valid_profile_name(profile) {
        eprintln!("[tray] rejecting menu id with invalid profile: {:?}", id);
        return None;
    }

    match verb {
        "start" => Some(MenuAction::StartInstance(profile.to_string())),
        "stop" => Some(MenuAction::StopInstance(profile.to_string())),
        _ => None,
    }
}

/// Render a 32x32 status glyph as RGBA.
///
/// States differ by *shape*, not colour: on macOS these are template images,
/// which are drawn as a monochrome mask so the menu bar can invert them for
/// light and dark. A colour-coded icon would come out identical in all states.
fn status_icon(state: TrayState) -> tauri::image::Image<'static> {
    const SIZE: u32 = 32;
    let centre = (SIZE as f32 - 1.0) / 2.0;
    let outer = 11.0_f32;

    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - centre;
            let dy = y as f32 - centre;
            let dist = (dx * dx + dy * dy).sqrt();

            let inside = match state {
                // Filled disc.
                TrayState::Running => dist <= outer,
                // Ring — clearly "not filled" at a glance.
                TrayState::Stopped => dist <= outer && dist >= outer - 3.0,
                // Small dot: mid-way between the two, reads as "working".
                TrayState::Pending => dist <= outer * 0.45,
            };

            // Template images carry shape in the alpha channel; the colour
            // channels are ignored by macOS.
            let alpha = if inside { 255 } else { 0 };
            rgba.extend_from_slice(&[0, 0, 0, alpha]);
        }
    }

    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

/// The project's own icon (bundle icons from tauri.conf.json) — visible and
/// on-brand, unlike the generated status glyph which can render invisible on
/// macOS template mode. Falls back to the status glyph if no icon is bundled.
fn project_icon(app: &AppHandle) -> tauri::image::Image<'static> {
    match app.default_window_icon() {
        Some(img) => {
            // Re-buffer as owned: default_window_icon borrows from the app
            // handle, but the tray needs an Image<'static>.
            tauri::image::Image::new_owned(img.rgba().to_vec(), img.width(), img.height())
        }
        None => status_icon(TrayState::Stopped),
    }
}

/// Build the menu for the current instance list.
fn build_menu(
    app: &AppHandle,
    instances: &[ColimaInstance],
    pending: &HashSet<String>,
) -> tauri::Result<Menu<tauri::Wry>> {
    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = Vec::new();

    // OrbStack-style: list running containers first (from the docker watcher's
    // cache — refreshed every poller tick alongside the instances).
    let containers = {
        let state = app.state::<Arc<RwLock<crate::docker_state::DockerState>>>();
        let result = match state.try_read() {
            Ok(guard) => guard
                .containers_cache
                .iter()
                .filter(|c| c.get("State").and_then(|s| s.as_str()) == Some("running"))
                .filter_map(|c| {
                    let name = c.get("Names").and_then(|n| n.as_str()).unwrap_or("");
                    let image = c.get("Image").and_then(|i| i.as_str()).unwrap_or("");
                    if name.is_empty() {
                        None
                    } else {
                        Some((name.to_string(), image.to_string()))
                    }
                })
                .take(10)
                .collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };
        result
    };
    if !containers.is_empty() {
        items.push(Box::new(MenuItem::with_id(
            app,
            "header:containers",
            "Running Containers",
            false,
            None::<&str>,
        )?));
        for (name, image) in &containers {
            let label = if image.is_empty() {
                name.clone()
            } else {
                format!("{}  ({})", name, image)
            };
            items.push(Box::new(MenuItem::with_id(
                app,
                format!("container:open:{}", name),
                label,
                true,
                None::<&str>,
            )?));
        }
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
    }

    if instances.is_empty() {
        let none = MenuItem::with_id(app, "noop:none", "No instances", false, None::<&str>)?;
        items.push(Box::new(none));
    } else {
        for instance in instances {
            let running = instance.status.eq_ignore_ascii_case("running");
            let busy = pending.contains(&instance.name);

            let label = if busy {
                format!("{} — working…", instance.name)
            } else if running {
                format!("{} — running", instance.name)
            } else {
                format!("{} — stopped", instance.name)
            };

            let id = if running {
                format!("instance:stop:{}", instance.name)
            } else {
                format!("instance:start:{}", instance.name)
            };

            // Disabled while an operation is in flight: this is what stops a
            // second click on a stale menu from firing a second command.
            items.push(Box::new(MenuItem::with_id(
                app, id, label, !busy, None::<&str>,
            )?));
        }
    }

    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(MenuItem::with_id(
        app, "show", "Open ColimaUI", true, None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        app, "help", "Help", true, None::<&str>,
    )?));
    // Offered only when something could actually be acting: the switch is on,
    // and a rule is set to act by itself. An entry reading "Stop self-healing"
    // on an install where nothing can act invites the user to fix a problem
    // they do not have.
    let can_act = crate::commands::self_heal::is_enabled()
        && crate::commands::self_heal::list_rules()
            .map(|rules| rules.iter().any(|r| r.enabled && r.mode == crate::commands::self_heal::HealMode::Auto))
            .unwrap_or(false);
    if can_act {
        items.push(Box::new(MenuItem::with_id(
            app,
            "self-heal:stop",
            "Stop self-healing",
            true,
            None::<&str>,
        )?));
    }
    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(MenuItem::with_id(
        app, "quit", "Quit", true, None::<&str>,
    )?));

    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|i| i.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Show and focus the main window.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    }
}

/// Mark a profile busy, run `op`, then clear it and refresh the tray.
fn run_instance_op<F>(app: &AppHandle, profile: String, op: F)
where
    F: std::future::Future<Output = Result<String, crate::error::ColimaError>> + Send + 'static,
{
    if let Ok(mut guard) = PENDING.lock() {
        // Already working on this one — a stale menu produced a duplicate.
        if !guard.insert(profile.clone()) {
            return;
        }
    }
    refresh_from_state(app);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = op.await {
            eprintln!("[tray] instance operation failed: {}", e);
        }
        if let Ok(mut guard) = PENDING.lock() {
            guard.remove(&profile);
        }
        // Force the next refresh to rebuild even if the poller has not caught
        // up with the new status yet.
        if let Ok(mut guard) = LAST_SNAPSHOT.lock() {
            *guard = None;
        }
        refresh_from_state(&handle);
    });
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    let Some(action) = parse_menu_id(id) else {
        return;
    };

    match action {
        MenuAction::ShowWindow => show_main_window(app),
        MenuAction::Quit => app.exit(0),
        MenuAction::Help => {
            show_main_window(app);
            let _ = app.emit("navigate", serde_json::json!({ "page": "help" }));
        }
        MenuAction::StopSelfHealing => {
            // Written before anything is reported, so the switch has taken
            // effect by the time the menu redraws without the entry.
            if let Err(e) = crate::commands::self_heal::set_enabled(false) {
                eprintln!("[Tray] could not stop self-healing: {e}");
            }
            let _ = app.emit("self-heal-stopped", serde_json::json!({}));
        }
        MenuAction::OpenContainer(_name) => {
            // Opening a container means jumping into the app's Containers page.
            show_main_window(app);
            let _ = app.emit("navigate", serde_json::json!({ "page": "containers" }));
        }
        MenuAction::StartInstance(profile) => {
            let p = profile.clone();
            run_instance_op(app, profile, async move {
                crate::commands::colima::start_instance_cli(p).await
            });
        }
        MenuAction::StopInstance(profile) => {
            let p = profile.clone();
            run_instance_op(app, profile, async move {
                crate::commands::colima::stop_instance_cli(p, false).await
            });
        }
    }
}

/// Rebuild the tray from the instance list the poller holds.
pub fn refresh_from_state(app: &AppHandle) {
    let instances = {
        let state = app.state::<crate::poller::PollerState>();
        let Ok(guard) = state.instances.try_lock() else {
            // The poller is mid-update; the next tick will refresh us.
            return;
        };
        guard.clone()
    };
    refresh(app, &instances);
}

/// Apply an instance list to the tray, skipping the rebuild when nothing that
/// the menu shows has changed.
pub fn refresh(app: &AppHandle, instances: &[ColimaInstance]) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    let pending = PENDING.lock().map(|g| g.clone()).unwrap_or_default();
    let snapshot = snapshot_of(instances);

    let changed = {
        let mut guard = match LAST_SNAPSHOT.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        // Pending state is part of what the menu renders, so a change there has
        // to force a rebuild too.
        let key = (snapshot.clone(), {
            let mut p: Vec<_> = pending.iter().cloned().collect();
            p.sort();
            p
        });
        let previous = guard.take();
        let differs = previous.as_ref() != Some(&key.0) || !pending.is_empty();
        *guard = Some(key.0);
        differs
    };

    if !changed {
        return;
    }

    if let Ok(menu) = build_menu(app, instances, &pending) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_icon(Some(project_icon(app)));
}

/// Create the tray icon.
///
/// Failure is not fatal: several Linux desktops have no system tray, and the
/// app is perfectly usable without one.
pub fn init(app: &AppHandle) {
    let menu = match build_menu(app, &[], &HashSet::new()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[tray] could not build menu, tray disabled: {}", e);
            return;
        }
    };

    let result = TrayIconBuilder::with_id(TRAY_ID)
        .icon(project_icon(app))
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            // Left- and right-click both toggle the window (OrbStack-style);
            // the button field distinguishes them on every platform.
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button_state == tauri::tray::MouseButtonState::Up
                    && matches!(
                        button,
                        tauri::tray::MouseButton::Left | tauri::tray::MouseButton::Right
                    )
                {
                    toggle_main_window(tray.app_handle());
                }
            }
        })
        .build(app);

    match result {
        Ok(_) => refresh_from_state(app),
        Err(e) => {
            eprintln!("[tray] system tray unavailable, continuing without it: {}", e);
            return;
        }
    }

    // The preference lives in SQLite, which is only readable asynchronously, so
    // the tray is created first and withdrawn if the user turned it off. Doing
    // it the other way round would mean either blocking startup on a DB read or
    // waiting for the frontend — and `settingsStore.svelte.ts` is not available
    // this early.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let show = crate::commands::knowledge_bank::get_setting(SHOW_TRAY_SETTING.to_string())
            .await
            .ok()
            .flatten();
        // Absent means "not configured yet" — default to showing it.
        if show.as_deref() == Some("false") {
            handle.remove_tray_by_id(TRAY_ID);
        }
    });
}

/// Unused today, kept so callers do not reach into `PENDING` directly.
#[allow(dead_code)]
pub fn pending_profiles() -> HashMap<String, ()> {
    PENDING
        .lock()
        .map(|g| g.iter().map(|p| (p.clone(), ())).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(name: &str, status: &str) -> ColimaInstance {
        ColimaInstance {
            name: name.to_string(),
            status: status.to_string(),
            arch: String::new(),
            cpus: 0,
            memory: 0,
            disk: 0,
            runtime: String::new(),
            address: String::new(),
            kubernetes: false,
        }
    }

    #[test]
    fn parses_valid_menu_ids() {
        assert_eq!(parse_menu_id("show"), Some(MenuAction::ShowWindow));
        assert_eq!(parse_menu_id("quit"), Some(MenuAction::Quit));
        assert_eq!(
            parse_menu_id("self-heal:stop"),
            Some(MenuAction::StopSelfHealing)
        );
        assert_eq!(
            parse_menu_id("instance:start:dev"),
            Some(MenuAction::StartInstance("dev".into()))
        );
        assert_eq!(
            parse_menu_id("instance:stop:default"),
            Some(MenuAction::StopInstance("default".into()))
        );
    }

    #[test]
    fn rejects_hostile_profile_names() {
        // Menu ids come back from the OS; a leading dash would be read by
        // colima as a flag, and `..` would escape ~/.colima.
        for id in [
            "instance:start:-rf",
            "instance:stop:../../etc",
            "instance:start:a b",
            "instance:start:",
            "instance:start:a\0b",
        ] {
            assert_eq!(parse_menu_id(id), None, "should reject: {id:?}");
        }
    }

    #[test]
    fn rejects_unknown_verbs_and_shapes() {
        assert_eq!(parse_menu_id("instance:delete:dev"), None);
        assert_eq!(parse_menu_id("instance:dev"), None);
        assert_eq!(parse_menu_id("nonsense"), None);
        assert_eq!(parse_menu_id(""), None);
    }

    #[test]
    fn aggregate_prefers_pending_then_running() {
        let running = vec![instance("dev", "Running")];
        let stopped = vec![instance("dev", "Stopped")];
        let empty = HashSet::new();
        let busy: HashSet<String> = ["dev".to_string()].into_iter().collect();

        assert_eq!(aggregate_state(&running, &empty), TrayState::Running);
        assert_eq!(aggregate_state(&stopped, &empty), TrayState::Stopped);
        assert_eq!(aggregate_state(&[], &empty), TrayState::Stopped);
        // An operation in flight outranks the last poll result.
        assert_eq!(aggregate_state(&running, &busy), TrayState::Pending);
        assert_eq!(aggregate_state(&stopped, &busy), TrayState::Pending);
    }

    #[test]
    fn status_matching_is_case_insensitive() {
        // colima reports "Running", the filesystem reader lowercases in places.
        for status in ["Running", "running", "RUNNING"] {
            assert_eq!(
                aggregate_state(&[instance("dev", status)], &HashSet::new()),
                TrayState::Running
            );
        }
    }

    #[test]
    fn snapshot_captures_name_and_status_only() {
        let a = snapshot_of(&[instance("dev", "Running")]);
        let b = snapshot_of(&[instance("dev", "Running")]);
        assert_eq!(a, b, "identical lists must produce an identical snapshot");

        let c = snapshot_of(&[instance("dev", "Stopped")]);
        assert_ne!(a, c, "a status change must be visible to the diff");
    }

    #[test]
    fn icons_differ_between_states() {
        let running = status_icon(TrayState::Running);
        let stopped = status_icon(TrayState::Stopped);
        let pending = status_icon(TrayState::Pending);
        assert_ne!(running.rgba(), stopped.rgba());
        assert_ne!(running.rgba(), pending.rgba());
        assert_ne!(stopped.rgba(), pending.rgba());
    }
}
