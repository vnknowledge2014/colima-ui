//! Terminal session manager backed by real pseudo-terminals.
//!
//! Previously each session was `script -q /dev/null ssh …` with piped stdio.
//! `script(1)` exists only to make the child believe it has a TTY, which buys
//! character echo and nothing else: no window size, no `SIGWINCH`, no job
//! control, and flag spellings that differ between BSD and GNU. Everything that
//! makes a terminal a terminal — resize, `Ctrl+C`, `vim` — needs an actual pty,
//! so this module opens one via `portable-pty` and spawns onto its slave.
//!
//! Output is buffered as **bytes**. The previous implementation decoded each
//! chunk with `from_utf8_lossy`, which corrupts any multi-byte character that
//! happens to straddle a read boundary. Callers that still want a `String` go
//! through [`SessionManager::read`], which only hands back whole characters and
//! keeps the trailing partial sequence for the next call.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};

use crate::validation::{ensure_valid_profile, is_valid_k8s_name};

/// Maximum buffered output per session (1 MB). A shell can produce output far
/// faster than the UI drains it, so the buffer is bounded and drops from the
/// oldest end rather than growing until the process is killed by the OS.
const MAX_BUFFER_BYTES: usize = 1024 * 1024;

/// How much to keep when the cap is hit. Trimming back to exactly the cap makes
/// every subsequent read shift the whole buffer — with `yes` running that is a
/// ~1 MB memmove per 8 KB chunk, under the mutex the reader and the UI share.
/// Dropping down to this instead amortises the cost to roughly nothing.
const BUFFER_TRIM_TO_BYTES: usize = 512 * 1024;

/// How often buffered output is pushed to the UI.
///
/// Sending every pty read straight through would put one IPC message per 8 KB
/// chunk on the wire; a build log would drown the webview in messages it cannot
/// render faster than one frame anyway. Coalescing on a frame boundary keeps
/// latency imperceptible while collapsing bursts into a single message.
const FLUSH_INTERVAL: Duration = Duration::from_millis(16);

/// Fallback grid used between `create` and the first resize from the frontend.
/// Nothing renders at 0×0 and some shells abort on it, so never start there.
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;

type OutputBuffer = Arc<Mutex<Vec<u8>>>;

/// A live pty plus the child attached to it.
pub struct TerminalSession {
    /// Master side only — see `create` for why the slave is not kept.
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    buffer: OutputBuffer,
    /// Cleared on close so the reader and flush threads unwind.
    running: Arc<AtomicBool>,
    /// Last time bytes moved in either direction. Shared with the flush thread
    /// so output counts as activity too — otherwise a `tail -f` left running
    /// would be reaped as idle while it is visibly working.
    last_activity: Arc<Mutex<Instant>>,
    closed: bool,
}

/// What a session is attached to.
///
/// All three spawn onto the same pty and share transport, resize, reaping and
/// the session cap; only the argv differs. Adding a fourth target should mean
/// adding a variant here, not a parallel code path.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionKind {
    Colima { profile: String },
    Lima { instance: String },
    K8sExec {
        namespace: String,
        pod: String,
        #[serde(default)]
        container: String,
    },
}

/// A resolved command line: program plus argv, never a shell string.
#[derive(Debug, PartialEq, Eq)]
pub struct Spawn {
    pub program: String,
    pub args: Vec<String>,
}

/// Per-profile history so `↑` survives across sessions and profiles do not
/// share a list.
///
/// `PROMPT_COMMAND='history -a'` matters because several tabs on one profile now
/// share this file: bash's default append-on-exit would keep only the last tab
/// to close. `HISTCONTROL=ignorespace` lets a user keep a secret out of the file
/// by prefixing the command with a space.
///
/// A login rc that assigns `HISTFILE` or `PROMPT_COMMAND` itself still wins —
/// there is no way to stop that from out here.
fn history_preamble(name: &str) -> String {
    format!(
        "mkdir -p \"$HOME/.colima-ui\"; \
         export HISTFILE=\"$HOME/.colima-ui/history-{name}\" \
         HISTCONTROL=ignorespace \
         PROMPT_COMMAND='history -a'; \
         exec \"$SHELL\" -l"
    )
}

/// Build the command for a session kind. Pure, so the injection guards below
/// are testable without opening a pty.
pub fn build_spawn(kind: &SessionKind, home: &str) -> Result<Spawn, String> {
    match kind {
        SessionKind::Colima { profile } | SessionKind::Lima { instance: profile } => {
            ensure_valid_profile(profile)?;
            let name = if profile.is_empty() { "default" } else { profile };

            let (ssh_config, host) = match kind {
                SessionKind::Lima { .. } => (
                    format!("{home}/.lima/{name}/ssh.config"),
                    format!("lima-{name}"),
                ),
                _ => {
                    let lima_name = if name == "default" {
                        "colima".to_string()
                    } else {
                        format!("colima-{name}")
                    };
                    (
                        format!("{home}/.colima/_lima/{lima_name}/ssh.config"),
                        format!("lima-{lima_name}"),
                    )
                }
            };

            Ok(Spawn {
                program: "ssh".to_string(),
                args: vec![
                    "-tt".to_string(),
                    "-o".to_string(),
                    "LogLevel=QUIET".to_string(),
                    "-F".to_string(),
                    ssh_config,
                    host,
                    history_preamble(name),
                ],
            })
        }

        SessionKind::K8sExec {
            namespace,
            pod,
            container,
        } => {
            // Same validators the k8s HTTP routes use — these land in argv.
            if !is_valid_k8s_name(namespace) {
                return Err(format!("Invalid namespace: {namespace}"));
            }
            if !is_valid_k8s_name(pod) {
                return Err(format!("Invalid pod name: {pod}"));
            }
            if !container.is_empty() && !is_valid_k8s_name(container) {
                return Err(format!("Invalid container name: {container}"));
            }

            let mut args = vec![
                "exec".to_string(),
                "-it".to_string(),
                "-n".to_string(),
                namespace.clone(),
                pod.clone(),
            ];
            if !container.is_empty() {
                args.push("-c".to_string());
                args.push(container.clone());
            }
            // Prefer bash, fall back to sh.
            //
            // Note what this does *not* cover: the chain is evaluated by `sh`,
            // so it only helps an image that already has `sh`. An image built
            // FROM scratch — coredns, and most distroless bases — has no shell
            // at all, and kubectl exits 127 with a raw OCI error. That case is
            // caught by exit-code, in `exitHint` on the frontend; there is no
            // argv that can conjure a shell into an image which lacks one.
            args.push("--".to_string());
            args.push("sh".to_string());
            args.push("-c".to_string());
            args.push(
                "command -v bash >/dev/null 2>&1 && exec bash -l || exec sh".to_string(),
            );

            Ok(Spawn {
                program: "kubectl".to_string(),
                args,
            })
        }
    }
}

pub struct SessionManager {
    sessions: HashMap<String, TerminalSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Open a session against a VM or a cluster container.
    ///
    /// `on_output` receives decoded text on a background thread, coalesced to
    /// one call per [`FLUSH_INTERVAL`]. Passing a sink is what replaced the old
    /// poll-the-buffer-over-HTTP loop.
    pub fn create(
        &mut self,
        session_id: &str,
        kind: &SessionKind,
        on_output: Box<dyn Fn(String) + Send + 'static>,
    ) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        // Validation lives in `build_spawn`, so it cannot be skipped by a caller
        // that builds a command some other way.
        let spawn = build_spawn(kind, &home)?;

        // Idempotent: a remount must not leave the previous pty orphaned.
        if self.sessions.contains_key(session_id) {
            let _ = self.close(session_id);
        }

        // Fail with something the user can act on rather than letting ssh emit
        // a config-file error into the terminal.
        if let Some(cfg) = spawn.args.iter().position(|a| a == "-F") {
            let path = &spawn.args[cfg + 1];
            if !std::path::Path::new(path).exists() {
                return Err(format!(
                    "SSH config not found: {path}. Is the instance running?"
                ));
            }
        }

        let pty = NativePtySystem::default();
        let pair = pty
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open pty: {e}"))?;

        // Built as argv, never as a shell string — nothing here is re-parsed by
        // a shell, so there is no quoting to get wrong.
        let mut cmd = CommandBuilder::new(&spawn.program);
        cmd.args(&spawn.args);
        cmd.env("TERM", "xterm-256color");

        let PtyPair { master, slave } = pair;

        let child = slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn terminal: {e}"))?;

        let mut reader = master
            .try_clone_reader()
            .map_err(|e| format!("Failed to read from pty: {e}"))?;
        let writer = master
            .take_writer()
            .map_err(|e| format!("Failed to write to pty: {e}"))?;

        // Release our handle on the slave the moment the child owns it. A pty
        // master only reports EOF once *every* slave fd is closed, so holding
        // this would leave the reader thread blocked forever after the user
        // types `exit` — one leaked thread and its buffer per abandoned session.
        drop(slave);

        // A pty merges stdout and stderr onto one stream, so one reader thread
        // replaces the two the piped version needed — and the interleaving is
        // now the kernel's, which is what the user actually typed at.
        let buffer: OutputBuffer = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));
        let last_activity = Arc::new(Mutex::new(Instant::now()));

        let buf_clone = Arc::clone(&buffer);
        thread::spawn(move || {
            let mut tmp = [0u8; 8192];
            loop {
                match reader.read(&mut tmp) {
                    Ok(0) => break, // pty closed
                    Ok(n) => {
                        let mut buf = buf_clone.lock().unwrap();
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.len() > MAX_BUFFER_BYTES {
                            // Note this cuts at an arbitrary byte, so an escape
                            // sequence or character can be severed. That is the
                            // price of bounded memory, and it only happens when
                            // the producer has already outrun the UI by 1 MB.
                            let excess = buf.len() - BUFFER_TRIM_TO_BYTES;
                            buf.drain(..excess);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Flush thread: drains the buffer on a frame boundary and pushes text
        // to the UI. It owns the carry, so a character split across two pty
        // reads is completed on the next tick instead of being replaced.
        let flush_buf = Arc::clone(&buffer);
        let flush_running = Arc::clone(&running);
        let flush_activity = Arc::clone(&last_activity);
        thread::spawn(move || {
            let mut carry: Vec<u8> = Vec::new();
            while flush_running.load(Ordering::Relaxed) {
                thread::sleep(FLUSH_INTERVAL);

                let chunk = {
                    let mut buf = flush_buf.lock().unwrap();
                    std::mem::take(&mut *buf)
                };
                if chunk.is_empty() {
                    continue;
                }

                let mut pending = std::mem::take(&mut carry);
                pending.extend_from_slice(&chunk);

                let (text, rest) = split_on_char_boundary(pending);
                carry = rest;
                if !text.is_empty() {
                    if let Ok(mut a) = flush_activity.lock() {
                        *a = Instant::now();
                    }
                    on_output(text);
                }
            }
        });

        self.sessions.insert(
            session_id.to_string(),
            TerminalSession {
                master,
                writer,
                child,
                buffer,
                running,
                last_activity,
                closed: false,
            },
        );

        Ok(())
    }

    /// Send input to the session.
    pub fn write(&mut self, session_id: &str, data: &str) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "Session not found".to_string())?;

        if session.closed {
            return Err("Session is closed".to_string());
        }

        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Write error: {e}"))?;
        session
            .writer
            .flush()
            .map_err(|e| format!("Flush error: {e}"))?;

        if let Ok(mut a) = session.last_activity.lock() {
            *a = Instant::now();
        }
        Ok(())
    }

    /// Close sessions with no traffic in either direction for `max_idle`.
    ///
    /// Multi-tab means nothing bounds pty count by construction: each tab is a
    /// real shell holding an ssh connection into the VM, and tabs are easy to
    /// open and forget. Returns the ids that were reaped so the caller can tell
    /// the UI.
    pub fn reap_idle(&mut self, max_idle: Duration) -> Vec<String> {
        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                s.last_activity
                    .lock()
                    .map(|a| a.elapsed() > max_idle)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in &stale {
            let _ = self.close(id);
        }
        stale
    }

    /// Tell the pty its new grid, which delivers `SIGWINCH` to the child.
    ///
    /// Without this a full-screen program keeps drawing into the size it saw at
    /// startup. Zero in either axis is rejected: it is never a real window and
    /// some shells abort on it.
    pub fn resize(&mut self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        if rows == 0 || cols == 0 {
            return Err("Refusing to resize to a zero-sized grid".to_string());
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "Session not found".to_string())?;

        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Resize error: {e}"))
    }

    /// Report the exit code once, if the child has died.
    ///
    /// Output no longer needs polling — the flush thread pushes it — but the UI
    /// still has to learn that the shell is gone, or it sits on a dead prompt.
    /// Returns `Some` exactly once per session.
    pub fn poll_exit(&mut self, session_id: &str) -> Option<u32> {
        let session = self.sessions.get_mut(session_id)?;
        if session.closed {
            return None;
        }
        match session.child.try_wait() {
            Ok(Some(status)) => {
                session.closed = true;
                Some(status.exit_code())
            }
            _ => None,
        }
    }

    /// Kill the session and reap it.
    ///
    /// `ssh -tt` is a single local process — the shell lives on the remote side
    /// and is hung up by sshd when the connection drops — so killing the child
    /// is enough and no process-group kill is needed. Dropping the master then
    /// closes the pty, which lets the reader thread see EOF and exit; clearing
    /// `running` unwinds the flush thread on its next tick.
    pub fn close(&mut self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.remove(session_id) {
            session.running.store(false, Ordering::Relaxed);
            let _ = session.child.kill();
            let _ = session.child.wait();
            drop(session.writer);
            drop(session.master);
        }
        Ok(())
    }

    /// Close every session. Called on app exit so no pty outlives the window.
    pub fn close_all(&mut self) {
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        for id in ids {
            let _ = self.close(&id);
        }
    }

    /// Number of live sessions. Used by the session cap.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether this id already has a session — a reconnect rather than a new tab.
    pub fn contains(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Split pty output into decodable text plus any truncated trailing character.
///
/// A pty read can end mid-character; decoding that chunk on its own turns both
/// halves into `U+FFFD`. The tail is handed back so the caller can prepend it to
/// the next chunk.
///
/// Bytes that are invalid rather than merely incomplete are *not* carried — a
/// tail can be at most 3 bytes short of the 4-byte maximum, so anything longer
/// is real corruption and is replaced instead of being pushed back forever.
fn split_on_char_boundary(data: Vec<u8>) -> (String, Vec<u8>) {
    let mut out = String::new();
    let mut rest = &data[..];

    // Loop rather than inspecting only the first error. `from_utf8` reports one
    // error at a time, so a single stray byte early in the chunk would otherwise
    // force the whole remainder — including a legitimately truncated character
    // at the end — down the lossy path. That is reachable in normal operation:
    // the buffer cap cuts at an arbitrary offset, so a chunk read after an
    // overflow routinely *begins* mid-character.
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                return (out, Vec::new());
            }
            Err(e) => {
                let valid = e.valid_up_to();
                out.push_str(std::str::from_utf8(&rest[..valid]).unwrap_or_default());

                match e.error_len() {
                    // Truncated: keep it for the next chunk to complete. A
                    // prefix is at most 3 bytes short of the 4-byte maximum.
                    None => return (out, rest[valid..].to_vec()),
                    // Genuinely invalid: replace and keep scanning, so bad bytes
                    // never stall the stream.
                    Some(bad) => {
                        out.push(char::REPLACEMENT_CHARACTER);
                        rest = &rest[valid + bad..];
                    }
                }
            }
        }
    }
}

pub type SharedSessionManager = Arc<Mutex<SessionManager>>;

/// Sessions with no traffic for this long are closed.
///
/// Generous on purpose: output counts as activity, so this only fires on a
/// session that is genuinely doing nothing. Killing a shell someone is reading
/// would be worse than the leak it prevents.
const MAX_IDLE: Duration = Duration::from_secs(30 * 60);

/// How often to look for idle sessions. Coarse — the deadline is 30 minutes.
const IDLE_SWEEP_INTERVAL: Duration = Duration::from_secs(60);

pub fn create_session_manager() -> SharedSessionManager {
    let mgr: SharedSessionManager = Arc::new(Mutex::new(SessionManager::new()));

    let sweeper = Arc::clone(&mgr);
    thread::spawn(move || loop {
        thread::sleep(IDLE_SWEEP_INTERVAL);
        if let Ok(mut m) = sweeper.lock() {
            m.reap_idle(MAX_IDLE);
        }
    });

    mgr
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/tester";

    #[test]
    fn rejects_hostile_profile_names() {
        // Each of these would otherwise reach a config path or an ssh argv
        // position, where a leading `-` is read as a flag. Empty is deliberately
        // absent — it means "default VM", see the test below.
        for bad in ["../../etc/passwd", "-oProxyCommand=touch /tmp/pwn", "a b"] {
            let kind = SessionKind::Colima {
                profile: bad.to_string(),
            };
            let e = build_spawn(&kind, HOME).unwrap_err();
            assert!(
                e.contains("Invalid profile name"),
                "expected {bad:?} to be rejected, got: {e}"
            );
        }
    }

    #[test]
    fn rejects_hostile_k8s_names() {
        let hostile = "-o/etc/passwd";
        for kind in [
            SessionKind::K8sExec {
                namespace: hostile.to_string(),
                pod: "p".into(),
                container: String::new(),
            },
            SessionKind::K8sExec {
                namespace: "ns".into(),
                pod: hostile.to_string(),
                container: String::new(),
            },
            SessionKind::K8sExec {
                namespace: "ns".into(),
                pod: "p".into(),
                container: hostile.to_string(),
            },
        ] {
            assert!(
                build_spawn(&kind, HOME).is_err(),
                "hostile k8s name must not reach argv: {kind:?}"
            );
        }
    }

    #[test]
    fn k8s_exec_builds_argv_not_a_shell_string() {
        let spawn = build_spawn(
            &SessionKind::K8sExec {
                namespace: "kube-system".into(),
                pod: "coredns-75d478d48f-6c4v2".into(),
                container: "coredns".into(),
            },
            HOME,
        )
        .unwrap();

        assert_eq!(spawn.program, "kubectl");
        // Every user-supplied value is its own argv element, so nothing is
        // re-parsed and there is no quoting to get wrong.
        assert!(spawn.args.contains(&"kube-system".to_string()));
        assert!(spawn.args.contains(&"coredns-75d478d48f-6c4v2".to_string()));
        assert!(spawn.args.contains(&"coredns".to_string()));
        assert!(spawn.args.contains(&"-it".to_string()));
    }

    #[test]
    fn k8s_exec_omits_container_flag_when_unspecified() {
        let spawn = build_spawn(
            &SessionKind::K8sExec {
                namespace: "default".into(),
                pod: "p".into(),
                container: String::new(),
            },
            HOME,
        )
        .unwrap();

        // Only the args before `--` are kubectl's; `-c` also appears after it as
        // part of `sh -c`, so a naive `contains` check passes for the wrong
        // reason.
        let sep = spawn.args.iter().position(|a| a == "--").unwrap();
        assert!(
            !spawn.args[..sep].contains(&"-c".to_string()),
            "no container flag should be passed when none was requested"
        );
    }

    #[test]
    fn colima_and_lima_resolve_different_ssh_configs() {
        let colima = build_spawn(
            &SessionKind::Colima {
                profile: "default".into(),
            },
            HOME,
        )
        .unwrap();
        let lima = build_spawn(
            &SessionKind::Lima {
                instance: "ubuntu".into(),
            },
            HOME,
        )
        .unwrap();

        assert!(colima.args.iter().any(|a| a.contains(".colima/_lima/colima/")));
        assert!(lima.args.iter().any(|a| a.contains(".lima/ubuntu/")));
    }

    #[test]
    fn history_file_is_per_profile() {
        let a = build_spawn(&SessionKind::Colima { profile: "a".into() }, HOME).unwrap();
        let b = build_spawn(&SessionKind::Colima { profile: "b".into() }, HOME).unwrap();

        let hist = |s: &Spawn| s.args.last().unwrap().clone();
        assert!(hist(&a).contains("history-a"));
        assert!(hist(&b).contains("history-b"));
        assert_ne!(hist(&a), hist(&b), "profiles must not share a history file");
        // Multiple tabs on one profile share the file, so appending per prompt
        // is what stops all but the last-closed tab losing its history.
        assert!(hist(&a).contains("history -a"));
    }

    #[test]
    fn refuses_zero_sized_resize() {
        let mut m = SessionManager::new();
        assert!(m.resize("missing", 0, 80).is_err());
        assert!(m.resize("missing", 24, 0).is_err());
    }

    #[test]
    fn whole_characters_decode_untouched() {
        let (text, carry) = split_on_char_boundary("hello ☃".as_bytes().to_vec());
        assert_eq!(text, "hello ☃");
        assert!(carry.is_empty());
    }

    #[test]
    fn truncated_character_is_carried_not_mangled() {
        let full = "ab☃".as_bytes().to_vec();
        // Cut one byte off the snowman: the old lossy decode produced "ab\u{FFFD}".
        let split = full[..full.len() - 1].to_vec();

        let (text, carry) = split_on_char_boundary(split);
        assert_eq!(text, "ab");
        assert_eq!(carry, vec![0xE2, 0x98]);

        // Feeding the carry back with the missing byte reconstitutes the char.
        let mut next = carry;
        next.push(0x83);
        let (rest, carry2) = split_on_char_boundary(next);
        assert_eq!(rest, "☃");
        assert!(carry2.is_empty());
    }

    /// Regression: an invalid byte *before* a truncated character used to
    /// disable the carry for the rest of the chunk, because `from_utf8` only
    /// reports its first error. The buffer cap cuts at an arbitrary offset, so
    /// chunks that start with a broken byte and end mid-character are normal
    /// after an overflow — not a contrived input.
    #[test]
    fn invalid_byte_does_not_disable_carry_for_later_truncation() {
        let (text, carry) = split_on_char_boundary(vec![0xFF, b'a', 0xE2, 0x98]);
        assert_eq!(text, "\u{FFFD}a");
        assert_eq!(
            carry,
            vec![0xE2, 0x98],
            "truncated tail must still be carried when an invalid byte precedes it"
        );
    }

    #[test]
    fn empty_profile_means_default_not_rejected() {
        // Empty is "the default VM" everywhere else in the codebase, so it must
        // resolve rather than error.
        let spawn = build_spawn(&SessionKind::Colima { profile: String::new() }, HOME).unwrap();
        assert!(spawn.args.iter().any(|a| a.contains("_lima/colima/")));
    }

    #[test]
    fn invalid_bytes_are_replaced_rather_than_carried_forever() {
        let (text, carry) = split_on_char_boundary(vec![b'o', b'k', 0xFF, 0xFE, b'!']);
        assert!(text.starts_with("ok"));
        assert!(
            carry.is_empty(),
            "invalid bytes must not be pushed back, or the stream stalls"
        );
    }
}
