//! Tauri commands for interactive terminal sessions.
//!
//! These replaced `/api/terminal/*`. The HTTP version needed a token on every
//! keystroke, polled for output every 100 ms, and exposed a shell on a local
//! port. Over IPC there is no port to reach, no handshake to authenticate, and
//! output is pushed instead of polled.

use tauri::ipc::Channel;
use tauri::State;

use crate::terminal_session::{SessionKind, SharedSessionManager};

/// How many sessions may be open at once.
///
/// Each tab is a real shell inside the VM holding an ssh connection, and tabs
/// are cheap to open by accident. Without a ceiling a long session of clicking
/// quietly exhausts the VM.
const MAX_SESSIONS: usize = 16;

/// Note which session was opened, so the UI can offer "reopen last session".
///
/// Metadata only — kind and target. Session *content* is deliberately never
/// stored or logged: people type credentials into shells.
fn record_session_opened(kind: &SessionKind) {
    let (k, target) = match kind {
        SessionKind::Colima { profile } => ("colima", profile.clone()),
        SessionKind::Lima { instance } => ("lima", instance.clone()),
        SessionKind::K8sExec {
            namespace,
            pod,
            container,
        } => (
            "k8sExec",
            if container.is_empty() {
                format!("{namespace}/{pod}")
            } else {
                format!("{namespace}/{pod}/{container}")
            },
        ),
    };

    // Best-effort: failing to write history must never stop a shell opening.
    if let Ok(conn) = crate::commands::knowledge_bank::get_db().lock() {
        let _ = conn.execute(
            "INSERT INTO terminal_sessions (kind, target) VALUES (?1, ?2)",
            rusqlite::params![k, target],
        );
    }
}

/// Open a session and start streaming its output into `on_output`.
#[tauri::command]
pub async fn terminal_create(
    mgr: State<'_, SharedSessionManager>,
    session_id: String,
    kind: SessionKind,
    on_output: Channel<String>,
) -> Result<(), crate::error::ColimaError> {
    let mut m = mgr.lock().unwrap();

    // Re-creating an existing id is a reconnect, not a new session, so it must
    // not count against the cap — otherwise a user at the ceiling can no longer
    // reattach to the tabs they already have.
    if !m.contains(&session_id) && m.len() >= MAX_SESSIONS {
        return Err(crate::error::ColimaError::from(format!(
            "Too many terminal sessions open ({MAX_SESSIONS}). Close one first."
        )));
    }

    record_session_opened(&kind);

    m.create(
        &session_id,
        &kind,
        Box::new(move |text| {
            // A send failure means the webview dropped the channel — the tab is
            // gone. Nothing useful to do here; `terminal_close` does the reaping.
            let _ = on_output.send(text);
        }),
    )
    .map_err(crate::error::ColimaError::from)
}

#[tauri::command]
pub async fn terminal_write(
    mgr: State<'_, SharedSessionManager>,
    session_id: String,
    data: String,
) -> Result<(), crate::error::ColimaError> {
    let mut m = mgr.lock().unwrap();
    m.write(&session_id, &data)
        .map_err(crate::error::ColimaError::from)
}

/// Push a new grid size, which delivers `SIGWINCH` to the child.
#[tauri::command]
pub async fn terminal_resize(
    mgr: State<'_, SharedSessionManager>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), crate::error::ColimaError> {
    let mut m = mgr.lock().unwrap();
    m.resize(&session_id, rows, cols)
        .map_err(crate::error::ColimaError::from)
}

/// Report the shell's exit code once it has died, so the UI stops pretending
/// it is connected. Output itself is pushed, not polled.
#[tauri::command]
pub async fn terminal_poll_exit(
    mgr: State<'_, SharedSessionManager>,
    session_id: String,
) -> Result<Option<u32>, crate::error::ColimaError> {
    let mut m = mgr.lock().unwrap();
    Ok(m.poll_exit(&session_id))
}

#[tauri::command]
pub async fn terminal_close(
    mgr: State<'_, SharedSessionManager>,
    session_id: String,
) -> Result<(), crate::error::ColimaError> {
    let mut m = mgr.lock().unwrap();
    m.close(&session_id).map_err(crate::error::ColimaError::from)
}
