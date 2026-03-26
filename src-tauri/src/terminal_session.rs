//! HTTP-based terminal session manager for browser mode.
//!
//! Uses background reader threads that continuously buffer stdout/stderr,
//! avoiding the fragile non-blocking fd manipulation approach.

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

/// Thread-safe output buffer shared between reader thread and main thread.
type OutputBuffer = Arc<Mutex<Vec<u8>>>;

/// A terminal session wrapping a child process with background readers.
pub struct TerminalSession {
    child: Child,
    buffer: OutputBuffer,
    closed: bool,
}

/// Manager holding all active terminal sessions.
pub struct SessionManager {
    sessions: HashMap<String, TerminalSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new terminal session by connecting via SSH.
    /// `vm_type` can be "colima" (default) or "lima" for standalone Lima VMs.
    pub fn create(&mut self, session_id: &str, profile: &str, vm_type: &str) -> Result<(), String> {
        // Idempotent: close existing session if any (handles React StrictMode double-mount)
        if self.sessions.contains_key(session_id) {
            let _ = self.close(session_id);
        }

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        
        let (ssh_config, host) = if vm_type == "lima" {
            // Standalone Lima VM: ~/.lima/<name>/ssh.config, host = lima-<name>
            let config = format!("{}/.lima/{}/ssh.config", home, profile);
            let h = format!("lima-{}", profile);
            (config, h)
        } else {
            // Colima instance: ~/.colima/_lima/<name>/ssh.config
            let lima_name = if profile == "default" {
                "colima".to_string()
            } else {
                format!("colima-{}", profile)
            };
            let config = format!("{}/.colima/_lima/{}/ssh.config", home, lima_name);
            let h = format!("lima-{}", lima_name);
            (config, h)
        };

        if !std::path::Path::new(&ssh_config).exists() {
            return Err(format!(
                "SSH config not found: {}. Is the instance running?",
                ssh_config
            ));
        }

        // Use `script` to create a real PTY wrapper around SSH.
        // Without this, SSH's stdout is a pipe, causing character echo to be
        // buffered until newline. With `script`, SSH sees a real terminal and
        // echoes each typed character immediately.
        let mut child = Command::new("script")
            .args([
                "-q",
                "/dev/null",
                "ssh",
                "-tt",
                "-o",
                "LogLevel=QUIET",
                "-o",
                "SetEnv=TERM=xterm-256color",
                "-F",
                &ssh_config,
                &host,
            ])
            .env("TERM", "xterm-256color")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn terminal: {}", e))?;

        // Shared buffer for output
        let buffer: OutputBuffer = Arc::new(Mutex::new(Vec::new()));

        // Spawn background thread to read stdout
        let stdout = child.stdout.take();
        let buf_clone = Arc::clone(&buffer);
        thread::spawn(move || {
            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout);
                let mut tmp = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut reader, &mut tmp) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let mut buf = buf_clone.lock().unwrap();
                            buf.extend_from_slice(&tmp[..n]);
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        // Spawn background thread to read stderr
        let stderr = child.stderr.take();
        let buf_clone2 = Arc::clone(&buffer);
        thread::spawn(move || {
            if let Some(stderr) = stderr {
                let mut reader = BufReader::new(stderr);
                let mut tmp = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut reader, &mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut buf = buf_clone2.lock().unwrap();
                            buf.extend_from_slice(&tmp[..n]);
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        self.sessions.insert(
            session_id.to_string(),
            TerminalSession {
                child,
                buffer,
                closed: false,
            },
        );

        Ok(())
    }

    /// Write data to a session's stdin.
    pub fn write(&mut self, session_id: &str, data: &str) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "Session not found".to_string())?;

        if session.closed {
            return Err("Session is closed".to_string());
        }

        if let Some(ref mut stdin) = session.child.stdin {
            stdin
                .write_all(data.as_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
            stdin.flush().map_err(|e| format!("Flush error: {}", e))?;
        } else {
            return Err("No stdin available".to_string());
        }

        Ok(())
    }

    /// Read buffered output (drains the buffer).
    pub fn read(&mut self, session_id: &str) -> Result<String, String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "Session not found".to_string())?;

        // Drain the shared buffer
        let data = {
            let mut buf = session.buffer.lock().unwrap();
            let data = buf.clone();
            buf.clear();
            data
        };

        // Check if process has exited
        if let Ok(Some(status)) = session.child.try_wait() {
            if !session.closed {
                session.closed = true;
                let exit_msg = format!(
                    "\r\n\x1b[33m● Session ended (exit code: {:?})\x1b[0m\r\n",
                    status.code()
                );
                let mut combined = data;
                combined.extend_from_slice(exit_msg.as_bytes());
                return Ok(String::from_utf8_lossy(&combined).to_string());
            }
        }

        Ok(String::from_utf8_lossy(&data).to_string())
    }

    /// Close a session and kill the process.
    pub fn close(&mut self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.remove(session_id) {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
        Ok(())
    }
}

/// Thread-safe wrapper for the session manager.
pub type SharedSessionManager = Arc<Mutex<SessionManager>>;

pub fn create_session_manager() -> SharedSessionManager {
    Arc::new(Mutex::new(SessionManager::new()))
}
