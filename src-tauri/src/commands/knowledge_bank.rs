use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// ===== Global DB Connection =====

static DB: std::sync::OnceLock<Mutex<Connection>> = std::sync::OnceLock::new();

fn db() -> &'static Mutex<Connection> {
    DB.get()
        .expect("Knowledge bank not initialized. Call init_knowledge_bank() first.")
}

pub fn get_db() -> &'static Mutex<Connection> {
    db()
}

/// Initialize the knowledge bank — called once from lib.rs setup()
pub fn init_knowledge_bank() {
    let dir = crate::path_util::app_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("knowledge.db");

    let conn = Connection::open(&db_path).expect("Failed to open knowledge.db");

    // Create tables
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS solutions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            error_pattern TEXT NOT NULL,
            error_category TEXT NOT NULL,
            solution_text TEXT NOT NULL,
            root_cause TEXT DEFAULT '',
            commands TEXT DEFAULT '[]',
            likes INTEGER DEFAULT 0,
            dislikes INTEGER DEFAULT 0,
            source TEXT DEFAULT 'builtin',
            created_at TEXT DEFAULT (datetime('now')),
            last_used_at TEXT
        );

        CREATE TABLE IF NOT EXISTS anti_patterns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            error_pattern TEXT NOT NULL,
            bad_suggestion TEXT NOT NULL,
            reason TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS agent_memory (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS agent_memory_fts USING fts5(
            content,
            content='agent_memory',
            content_rowid='rowid'
        );

        -- One row per chat thread in the AI panel. Messages point at it via
        -- `chat_messages.conversation_id`, so clearing one thread leaves the
        -- others intact.
        CREATE TABLE IF NOT EXISTS chat_conversations (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            conversation_id TEXT NOT NULL DEFAULT 'default'
        );

        -- Which terminal sessions were opened, never what was typed in them.
        -- Enough to offer reopening a recent session; storing content would mean
        -- storing whatever credentials the user pasted into a shell.
        CREATE TABLE IF NOT EXISTS terminal_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            target TEXT NOT NULL,
            started_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_terminal_sessions_started
            ON terminal_sessions(started_at DESC);

        CREATE TABLE IF NOT EXISTS app_settings (
            setting_key TEXT PRIMARY KEY,
            setting_value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS user_presets (
            id TEXT PRIMARY KEY,
            config_json TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS preset_container_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            preset_id TEXT NOT NULL,
            instance_profile TEXT NOT NULL,
            snapshot_time INTEGER NOT NULL,
            containers_json TEXT NOT NULL,
            is_manual_override INTEGER DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_preset_snapshot ON preset_container_snapshots(preset_id, instance_profile);


        -- Help articles. Distinct from `solutions`, which holds short
        -- pattern→remedy pairs mined from VM start failures for the AI agent.
        -- Articles are long-form prose the user reads, addressed by the stable
        -- `doc_id` slug that `error.rs` and `system_capabilities.rs` emit.
        --
        -- (slug, locale) is the dedupe key, so re-seeding is an upsert rather
        -- than a duplicate. `version` gates that upsert: bumping it in
        -- ARTICLE_VERSION ships new content, leaving it alone preserves the row.
        CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            platform TEXT NOT NULL DEFAULT 'all',
            version INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT DEFAULT (datetime('now')),
            UNIQUE(slug, locale)
        );

        -- FTS5 previously covered only `agent_memory`; articles need their own
        -- index. External-content mode keeps the text stored once, in `articles`.
        CREATE VIRTUAL TABLE IF NOT EXISTS articles_fts USING fts5(
            title,
            body,
            content='articles',
            content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS articles_ai_fts AFTER INSERT ON articles
        BEGIN
            INSERT INTO articles_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;

        CREATE TRIGGER IF NOT EXISTS articles_ad_fts AFTER DELETE ON articles
        BEGIN
            INSERT INTO articles_fts(articles_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
        END;

        CREATE TRIGGER IF NOT EXISTS articles_au_fts AFTER UPDATE ON articles
        BEGIN
            INSERT INTO articles_fts(articles_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
            INSERT INTO articles_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;

        CREATE TRIGGER IF NOT EXISTS agent_memory_ai_fts AFTER INSERT ON agent_memory
        BEGIN
            INSERT INTO agent_memory_fts(rowid, content) VALUES (new.rowid, new.content);
        END;

        CREATE TRIGGER IF NOT EXISTS agent_memory_ad_fts AFTER DELETE ON agent_memory
        BEGIN
            INSERT INTO agent_memory_fts(agent_memory_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS agent_memory_au_fts AFTER UPDATE OF content ON agent_memory
        BEGIN
            INSERT INTO agent_memory_fts(agent_memory_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
            INSERT INTO agent_memory_fts(rowid, content) VALUES (new.rowid, new.content);
        END;
    ",
    )
    .expect("Failed to create knowledge bank tables");

    migrate_chat_conversations(&conn);

    // Seed builtin solutions (only if empty)
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM solutions WHERE source = 'builtin'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if count == 0 {
        seed_builtins(&conn);
    }

    // Help articles seed on every launch: the upsert is version-gated, so this
    // is a no-op until an app update ships new content.
    super::kb_articles::seed(&conn);

    DB.set(Mutex::new(conn))
        .expect("Knowledge bank already initialized");
}

/// Attach existing chat messages to a conversation.
///
/// `chat_messages` shipped without `conversation_id`, so an installed database
/// already holds rows that predate threads. `CREATE TABLE IF NOT EXISTS` never
/// touches those, hence the explicit column add — guarded, because SQLite has
/// no `ADD COLUMN IF NOT EXISTS` and re-running must be a no-op.
///
/// Existing messages are adopted by a single thread rather than dropped: the
/// user's whole history would otherwise vanish behind an empty conversation
/// list on first launch after the update.
fn migrate_chat_conversations(conn: &Connection) {
    let has_column = conn
        .prepare("SELECT conversation_id FROM chat_messages LIMIT 1")
        .is_ok();

    if !has_column {
        if let Err(e) = conn.execute(
            "ALTER TABLE chat_messages ADD COLUMN conversation_id TEXT NOT NULL DEFAULT 'default'",
            [],
        ) {
            eprintln!("chat_messages migration failed: {e}");
            return;
        }
    }

    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation
         ON chat_messages(conversation_id, created_at)",
        [],
    );

    let orphans: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chat_messages
             WHERE conversation_id NOT IN (SELECT id FROM chat_conversations)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if orphans > 0 {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO chat_conversations (id, title)
             SELECT DISTINCT conversation_id, '' FROM chat_messages",
            [],
        );
    }
}

// ===== Seed Data =====

fn seed_builtins(conn: &Connection) {
    let builtins: Vec<(&str, &str, &str, &str, &str)> = vec![
        // (error_pattern, category, solution, root_cause, commands_json)
        (
            r"failed to run attach disk.*in use",
            "vm_start",
            "The VM disk is locked by zombie processes from a previous crashed start.\n\n**Fix:**\n```bash\ncolima stop --force\npkill -f \"limactl usernet\"\npkill -f \"colima daemon\"\ncolima start\n```",
            "Zombie limactl usernet or colima daemon processes holding the VM disk file lock",
            r#"["colima stop --force","pkill -f \"limactl usernet\"","pkill -f \"colima daemon\"","colima start"]"#
        ),
        (
            r"error starting vm.*exit status 1",
            "vm_start",
            "Generic VM boot failure. Use the diagnostic tool to read `ha.stderr.log` for the specific error.\n\n**Common causes:**\n1. Disk lock (zombie processes)\n2. Stale PID/socket files\n3. VZ/QEMU binary issues\n\n**Start with:**\n```bash\ncolima stop --force\ncolima start --verbose\n```",
            "Multiple possible causes — need to check Lima VM logs for specifics",
            r#"["colima stop --force","colima start --verbose"]"#
        ),
        (
            r"boot.*timeout|timed out waiting",
            "vm_start",
            "The VM is taking too long to boot.\n\n**Possible causes:**\n- Low CPU/memory allocation\n- Slow disk I/O\n- VZ framework issue on older macOS\n\n**Fix:**\n```bash\ncolima stop\ncolima start --cpu 4 --memory 4\n```",
            "VM boot exceeds timeout, usually due to insufficient resources",
            r#"["colima stop","colima start --cpu 4 --memory 4"]"#
        ),
        (
            r"qemu.*failed|qemu.*not found|qemu.*error",
            "vm_start",
            "QEMU binary issue.\n\n**Fix:**\n```bash\nbrew reinstall qemu\ncolima start --vm-type qemu\n```",
            "QEMU binary missing, corrupted, or incompatible version",
            r#"["brew reinstall qemu","colima start --vm-type qemu"]"#
        ),
        (
            r"vz.*Virtualization|vz.*entitlement|Virtualization\.framework",
            "vm_start",
            "macOS Virtualization.framework error.\n\n**Fix — switch to QEMU:**\n```bash\ncolima stop\ncolima start --vm-type qemu\n```\nOr ensure macOS 13+ is installed for VZ support.",
            "VZ driver requires macOS 13+ and proper entitlements",
            r#"["colima stop","colima start --vm-type qemu"]"#
        ),
        (
            r"Cannot connect.*Docker daemon|docker.*daemon.*not running",
            "docker_socket",
            "Docker daemon is not running because Colima is not started.\n\n**Fix:**\n```bash\ncolima start\n```\n\nIf Colima is running but Docker is unreachable:\n```bash\nexport DOCKER_HOST=unix://$HOME/.colima/default/docker.sock\n```",
            "Docker socket not found — Colima VM not running or socket path misconfigured",
            r#"["colima start"]"#
        ),
        (
            r"docker\.sock.*permission denied|Got permission denied.*docker",
            "docker_socket",
            "Docker socket permission error.\n\n**Fix:**\n```bash\nsudo rm /var/run/docker.sock\ncolima start\n```",
            "Stale Docker socket with wrong permissions (often from Docker Desktop)",
            r#"["sudo rm /var/run/docker.sock","colima start"]"#
        ),
        (
            r"context.*deadline exceeded|connection.*timed out.*docker",
            "docker_socket",
            "Docker daemon is unresponsive.\n\n**Fix:**\n```bash\ncolima restart\n```",
            "Docker daemon inside VM is hung or overloaded",
            r#"["colima restart"]"#
        ),
        (
            r"disk.*in use by instance",
            "disk_lock",
            "VM disk file is locked by another process.\n\n**Fix:**\n```bash\ncolima stop --force\npkill -f \"limactl usernet\"\npkill -f \"colima daemon\"\n# Remove stale PID files\nrm -f ~/.colima/_lima/colima/*.pid\ncolima start\n```",
            "Stale PID files or zombie processes from a previous crash holding disk lock",
            r#"["colima stop --force","pkill -f \"limactl usernet\"","pkill -f \"colima daemon\"","colima start"]"#
        ),
        (
            r"lock.*acquired|flock.*failed",
            "disk_lock",
            "File lock is held by another process.\n\n**Fix:**\n```bash\ncolima stop --force\nps aux | grep -E 'lima|colima' | grep -v grep\n# Kill any lingering processes shown above\ncolima start\n```",
            "File lock held by a zombie Colima/Lima process",
            r#"["colima stop --force","colima start"]"#
        ),
        (
            r"kubernetes.*not enabled|kubernetes.*not running",
            "k8s",
            "Kubernetes is not enabled on this Colima instance.\n\n**Fix:**\n```bash\ncolima kubernetes start\n```\n\nOr start Colima with K8s:\n```bash\ncolima start --kubernetes\n```",
            "Kubernetes was not enabled at instance creation or was stopped",
            r#"["colima kubernetes start"]"#
        ),
        (
            r"connection refused.*6443|Unable to connect.*kubernetes",
            "k8s",
            "Kubernetes API server is unreachable.\n\n**Fix:**\n```bash\ncolima kubernetes start\n# Or reset if corrupted:\ncolima kubernetes delete\ncolima kubernetes start\n```",
            "K8s API server not running or kubeconfig pointing to wrong endpoint",
            r#"["colima kubernetes start"]"#
        ),
        (
            r"kubeconfig.*not found|KUBECONFIG.*missing",
            "k8s",
            "Kubeconfig file is missing.\n\n**Fix:**\n```bash\nexport KUBECONFIG=~/.colima/default/kubeconfig.yaml\nkubectl get nodes\n```",
            "KUBECONFIG environment variable not set to Colima's kubeconfig",
            r#"[]"#
        ),
        (
            r"address already in use|port.*already.*bound|EADDRINUSE",
            "network",
            "A port is already in use by another process.\n\n**Find the conflicting process:**\n```bash\nlsof -i :<PORT_NUMBER>\n```\n\nThen kill it or use a different port.",
            "Another process is binding to the same port",
            r#"[]"#
        ),
        (
            r"DNS.*resolve|name resolution|could not resolve host",
            "network",
            "DNS resolution is failing inside the VM.\n\n**Fix:**\n```bash\ncolima stop\ncolima start --dns 8.8.8.8 --dns 1.1.1.1\n```",
            "VM's DNS configuration is broken or ISP DNS is blocking",
            r#"["colima stop","colima start --dns 8.8.8.8 --dns 1.1.1.1"]"#
        ),
        (
            r"OOM|out of memory|killed.*memory|Cannot allocate memory",
            "resource",
            "The VM ran out of memory.\n\n**Fix — increase memory:**\n```bash\ncolima stop\ncolima start --memory 8\n```",
            "VM memory allocation too low for the workload",
            r#"["colima stop","colima start --memory 8"]"#
        ),
        (
            r"no space left on device|disk.*full",
            "resource",
            "VM disk is full.\n\n**Fix — prune Docker data:**\n```bash\ndocker system prune -a --volumes\n```\n\nOr increase disk size:\n```bash\ncolima stop\ncolima start --disk 100\n```",
            "Docker images/volumes/build cache filling up the VM disk",
            r#"["docker system prune -a --volumes"]"#
        ),
        (
            r"compose.*not found|docker-compose.*command not found",
            "compose",
            "Docker Compose is not installed.\n\n**Fix:**\n```bash\nbrew install docker-compose\n```\n\nOr use the Docker CLI plugin:\n```bash\ndocker compose version\n```",
            "docker-compose binary not installed or not in PATH",
            r#"["brew install docker-compose"]"#
        ),
        (
            r"manifest.*not found|platform.*not.*supported|no matching manifest",
            "image",
            "Image not found for your architecture.\n\n**Fix — check platform:**\n```bash\ndocker pull --platform linux/amd64 <image>\n```\n\nOr use an ARM-compatible image tag.",
            "Image tag doesn't exist or isn't available for arm64/amd64",
            r#"[]"#
        ),
        (
            r"mount.*read-only|virtio.*error|virtiofs",
            "volume",
            "VirtioFS mount issue.\n\n**Fix — switch to sshfs:**\n```bash\ncolima stop\ncolima start --mount-type sshfs\n```",
            "VirtioFS has compatibility issues with certain macOS versions",
            r#"["colima stop","colima start --mount-type sshfs"]"#
        ),
        (
            r"version mismatch|incompatible.*version|upgrade required",
            "upgrade",
            "Colima/Lima version mismatch.\n\n**Fix:**\n```bash\nbrew upgrade colima lima\ncolima delete\ncolima start\n```\n\n⚠️ `colima delete` will remove the VM and all Docker data.",
            "Colima and Lima binaries out of sync after partial upgrade",
            r#"["brew upgrade colima lima"]"#
        ),
        (
            r"limactl.*not found|lima.*not installed",
            "lima",
            "Lima is not installed.\n\n**Fix:**\n```bash\nbrew install lima\n```",
            "Lima binary missing — required dependency for Colima",
            r#"["brew install lima"]"#
        ),
    ];

    let mut stmt = conn.prepare(
        "INSERT INTO solutions (error_pattern, error_category, solution_text, root_cause, commands, source)
         VALUES (?1, ?2, ?3, ?4, ?5, 'builtin')"
    ).expect("Failed to prepare insert");

    for (pattern, category, solution, cause, commands) in builtins {
        let _ = stmt.execute(params![pattern, category, solution, cause, commands]);
    }
}

// ===== Data Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KBMatch {
    pub id: i64,
    pub error_pattern: String,
    pub error_category: String,
    pub solution_text: String,
    pub root_cause: String,
    pub commands: String,
    pub likes: i64,
    pub dislikes: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KBAntiPattern {
    pub id: i64,
    pub error_pattern: String,
    pub bad_suggestion: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KBQueryResult {
    pub solutions: Vec<KBMatch>,
    pub anti_patterns: Vec<KBAntiPattern>,
    pub context_text: String, // Pre-formatted for AI injection
}

// ===== Tauri Commands =====

/// Query knowledge bank for matching solutions + anti-patterns
#[tauri::command]
pub async fn kb_query(error_text: String) -> Result<KBQueryResult, crate::error::ColimaError> {
    async move {
    let error_lower = error_text.to_lowercase();
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;

    // Find matching solutions (simple keyword matching from error_pattern)
    let mut stmt = conn.prepare(
        "SELECT id, error_pattern, error_category, solution_text, root_cause, commands, likes, dislikes, source
         FROM solutions ORDER BY (likes - dislikes) DESC, likes DESC"
    ).map_err(|e| format!("DB query: {}", e))?;

    let all_solutions: Vec<KBMatch> = stmt
        .query_map([], |row| {
            Ok(KBMatch {
                id: row.get(0)?,
                error_pattern: row.get(1)?,
                error_category: row.get(2)?,
                solution_text: row.get(3)?,
                root_cause: row.get(4)?,
                commands: row.get(5)?,
                likes: row.get(6)?,
                dislikes: row.get(7)?,
                source: row.get(8)?,
            })
        })
        .map_err(|e| format!("DB map: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Match using regex patterns
    let mut solutions: Vec<KBMatch> = Vec::new();
    for sol in &all_solutions {
        if let Ok(re) = regex_lite::Regex::new(&format!("(?i){}", sol.error_pattern)) {
            if re.is_match(&error_lower) || re.is_match(&error_text) {
                solutions.push(sol.clone());
            }
        } else {
            // Fallback: simple substring match
            if error_lower.contains(&sol.error_pattern.to_lowercase()) {
                solutions.push(sol.clone());
            }
        }
    }

    // Find matching anti-patterns
    let mut ap_stmt = conn
        .prepare("SELECT id, error_pattern, bad_suggestion, reason FROM anti_patterns")
        .map_err(|e| format!("DB query: {}", e))?;

    let all_aps: Vec<KBAntiPattern> = ap_stmt
        .query_map([], |row| {
            Ok(KBAntiPattern {
                id: row.get(0)?,
                error_pattern: row.get(1)?,
                bad_suggestion: row.get(2)?,
                reason: row.get(3)?,
            })
        })
        .map_err(|e| format!("DB map: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let anti_patterns: Vec<KBAntiPattern> = all_aps
        .into_iter()
        .filter(|ap| {
            if let Ok(re) = regex_lite::Regex::new(&format!("(?i){}", ap.error_pattern)) {
                re.is_match(&error_lower) || re.is_match(&error_text)
            } else {
                error_lower.contains(&ap.error_pattern.to_lowercase())
            }
        })
        .collect();

    // Format context for AI injection
    let mut context = String::new();
    if !solutions.is_empty() {
        context.push_str("## 📚 Knowledge Bank — Previously Known Solutions\n\n");
        for (i, sol) in solutions.iter().take(3).enumerate() {
            let score = sol.likes - sol.dislikes;
            let badge = if sol.source == "learned" {
                "🧠 Learned"
            } else {
                "📖 Built-in"
            };
            context.push_str(&format!(
                "### Solution {} ({}, score: {}👍)\n**Category:** {}\n**Root Cause:** {}\n**Fix:**\n{}\n\n",
                i + 1, badge, score, sol.error_category, sol.root_cause, sol.solution_text
            ));
        }
    }
    if !anti_patterns.is_empty() {
        context.push_str("## ⚠️ Anti-Patterns — Approaches That Did NOT Work\n\n");
        for ap in anti_patterns.iter().take(3) {
            context.push_str(&format!(
                "- **DO NOT suggest:** {}\n  **Reason:** {}\n\n",
                ap.bad_suggestion.lines().next().unwrap_or(""),
                ap.reason
            ));
        }
    }

    // Update last_used_at for matched solutions
    for sol in &solutions {
        let _ = conn.execute(
            "UPDATE solutions SET last_used_at = datetime('now') WHERE id = ?1",
            params![sol.id],
        );
    }

    Ok(KBQueryResult {
        solutions,
        anti_patterns,
        context_text: context,
    })
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Record feedback (like/dislike) for a solution.
/// Uses two fixed, literal SQL statements (no string interpolation of SQL
/// fragments) so the query text can never be influenced by caller input.
#[tauri::command]
pub async fn kb_feedback(solution_id: i64, is_like: bool) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    if is_like {
        conn.execute(
            "UPDATE solutions SET likes = likes + 1 WHERE id = ?1",
            params![solution_id],
        )
        .map_err(|e| format!("DB update: {}", e))?;
        Ok("Solution liked".to_string())
    } else {
        conn.execute(
            "UPDATE solutions SET dislikes = dislikes + 1 WHERE id = ?1",
            params![solution_id],
        )
        .map_err(|e| format!("DB update: {}", e))?;
        Ok("Solution disliked".to_string())
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Save a new learned solution from AI response
#[tauri::command]
pub async fn kb_save_solution(     error_pattern: String,     error_category: String,     solution_text: String,     root_cause: String, ) -> Result<i64, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO solutions (error_pattern, error_category, solution_text, root_cause, commands, source, likes)
         VALUES (?1, ?2, ?3, ?4, '[]', 'learned', 1)",
        params![error_pattern, error_category, solution_text, root_cause]
    ).map_err(|e| format!("DB insert: {}", e))?;
    Ok(conn.last_insert_rowid())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Agentic Context Engineering: Save a new learned solution directly from AI's [LEARN: ...] tool
#[tauri::command]
pub async fn kb_learn(error_pattern: String, solution_text: String) -> Result<i64, crate::error::ColimaError> {
    async move {
    kb_save_solution(
        error_pattern,
        "Agentic Learning".to_string(),
        solution_text,
        "Auto-distilled by reasoning loop".to_string(),
    )
    .await
    }
    .await
}

/// Save an anti-pattern (approach that didn't work)
#[tauri::command]
pub async fn kb_save_anti_pattern(     error_pattern: String,     bad_suggestion: String,     reason: String, ) -> Result<i64, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO anti_patterns (error_pattern, bad_suggestion, reason) VALUES (?1, ?2, ?3)",
        params![error_pattern, bad_suggestion, reason],
    )
    .map_err(|e| format!("DB insert: {}", e))?;
    Ok(conn.last_insert_rowid())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

fn stable_id(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(16);
    for byte in result.iter().take(8) {
        use std::fmt::Write;
        write!(&mut hex, "{:02x}", byte).unwrap();
    }
    hex
}

pub fn add_agent_memory(conn: &Connection, memory_type: &str, content: &str) -> rusqlite::Result<()> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let id = stable_id(&format!("mem|{}", nanos));
    conn.execute(
        "INSERT INTO agent_memory (id, type, content) VALUES (?1, ?2, ?3)",
        params![&id, memory_type, content],
    )?;
    Ok(())
}

pub fn search_agent_memory(conn: &Connection, query: &str, limit: u32) -> rusqlite::Result<Vec<String>> {
    let clean_query = query.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>();
    let tokens: Vec<&str> = clean_query.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    
    // e.g. "foo bar" -> "foo OR bar"
    let fts_query = tokens.join(" OR ");
    
    let mut stmt = conn.prepare(
        "SELECT content FROM agent_memory_fts 
         WHERE agent_memory_fts MATCH ?1 
         ORDER BY bm25(agent_memory_fts) 
         LIMIT ?2",
    )?;
    
    let results: rusqlite::Result<Vec<String>, _> = stmt
        .query_map(params![fts_query, limit], |row| row.get(0))?
        .collect();
        
    results
}

#[tauri::command]
pub async fn add_memory(memory_type: String, content: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    add_agent_memory(&conn, &memory_type, &content).map_err(|e| format!("DB insert: {}", e))?;
    Ok("Memory added".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn search_memory(query: String, limit: u32) -> Result<Vec<String>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    search_agent_memory(&conn, &query, limit).map_err(|e| format!("DB search: {}", e))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemoryItem {
    pub id: String,
    pub memory_type: String,
    pub content: String,
    pub created_at: i64,
}

#[tauri::command]
pub async fn get_all_memories() -> Result<Vec<AgentMemoryItem>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn
        .prepare("SELECT id, type, content, created_at FROM agent_memory ORDER BY created_at DESC")
        .map_err(|e| format!("DB prepare: {}", e))?;
    
    let results: Result<Vec<AgentMemoryItem>, _> = stmt
        .query_map([], |row| {
            Ok(AgentMemoryItem {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("DB query_map: {}", e))?
        .collect();

    results.map_err(|e| format!("DB collect: {}", e))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn update_memory(id: String, content: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "UPDATE agent_memory SET content = ?1 WHERE id = ?2",
        params![content, id],
    )
    .map_err(|e| format!("DB update: {}", e))?;
    Ok("Memory updated".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn delete_memory(id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "DELETE FROM agent_memory WHERE id = ?1",
        params![id],
    )
    .map_err(|e| format!("DB delete: {}", e))?;
    Ok("Memory deleted".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn save_preset_snapshot(     preset_id: String,     instance_profile: String,     containers_json: String,     is_manual_override: bool, ) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let snapshot_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO preset_container_snapshots 
         (preset_id, instance_profile, snapshot_time, containers_json, is_manual_override) 
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            preset_id,
            instance_profile,
            snapshot_time,
            containers_json,
            if is_manual_override { 1 } else { 0 }
        ],
    )
    .map_err(|e| format!("DB insert snapshot: {}", e))?;

    Ok("Snapshot saved".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSnapshot {
    pub id: i64,
    pub preset_id: String,
    pub instance_profile: String,
    pub snapshot_time: i64,
    pub containers_json: String,
    pub is_manual_override: bool,
}

#[tauri::command]
pub async fn load_preset_snapshot(     preset_id: String,     instance_profile: String, ) -> Result<Option<PresetSnapshot>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    
    // Get the most recent snapshot for this preset and instance profile
    let mut stmt = conn
        .prepare(
            "SELECT id, preset_id, instance_profile, snapshot_time, containers_json, is_manual_override 
             FROM preset_container_snapshots 
             WHERE preset_id = ?1 AND instance_profile = ?2 
             ORDER BY snapshot_time DESC LIMIT 1"
        )
        .map_err(|e| format!("DB prepare: {}", e))?;

    let mut iter = stmt
        .query_map(params![preset_id, instance_profile], |row| {
            Ok(PresetSnapshot {
                id: row.get(0)?,
                preset_id: row.get(1)?,
                instance_profile: row.get(2)?,
                snapshot_time: row.get(3)?,
                containers_json: row.get(4)?,
                is_manual_override: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| format!("DB query_map: {}", e))?;

    if let Some(Ok(snapshot)) = iter.next() {
        Ok(Some(snapshot))
    } else {
        Ok(None)
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Return the latest snapshot for every preset of a given instance profile.
/// Used by the Containers UI to build a container → preset ownership map.
#[tauri::command]
pub async fn list_all_preset_snapshots(     instance_profile: String, ) -> Result<Vec<PresetSnapshot>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;

    // For each preset_id, pick the row with the highest snapshot_time
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.preset_id, s.instance_profile, s.snapshot_time, s.containers_json, s.is_manual_override
             FROM preset_container_snapshots s
             INNER JOIN (
                 SELECT preset_id, MAX(snapshot_time) AS max_time
                 FROM preset_container_snapshots
                 WHERE instance_profile = ?1
                 GROUP BY preset_id
             ) latest ON s.preset_id = latest.preset_id AND s.snapshot_time = latest.max_time
             WHERE s.instance_profile = ?1"
        )
        .map_err(|e| format!("DB prepare: {}", e))?;

    let rows = stmt
        .query_map(params![instance_profile], |row| {
            Ok(PresetSnapshot {
                id: row.get(0)?,
                preset_id: row.get(1)?,
                instance_profile: row.get(2)?,
                snapshot_time: row.get(3)?,
                containers_json: row.get(4)?,
                is_manual_override: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| format!("DB query_map: {}", e))?;

    let results: Vec<PresetSnapshot> = rows.into_iter().flatten().collect();
    Ok(results)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

// ===== Settings & Presets (Migrated from LocalStorage) =====

#[tauri::command]
pub async fn get_setting(key: String) -> Result<Option<String>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare("SELECT setting_value FROM app_settings WHERE setting_key = ?1")
        .map_err(|e| format!("DB prepare: {}", e))?;
    let mut iter = stmt.query_map(params![key], |row| row.get(0))
        .map_err(|e| format!("DB query_map: {}", e))?;
    if let Some(Ok(val)) = iter.next() {
        Ok(Some(val))
    } else {
        Ok(None)
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn set_setting(key: String, value: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO app_settings (setting_key, setting_value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(setting_key) DO UPDATE SET setting_value = excluded.setting_value, updated_at = datetime('now')",
        params![key, value],
    )
    .map_err(|e| format!("DB execute: {}", e))?;
    Ok("Setting saved".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn get_all_settings() -> Result<std::collections::HashMap<String, String>, String> {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare("SELECT setting_key, setting_value FROM app_settings")
        .map_err(|e| format!("DB prepare: {}", e))?;
    let iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })
    .map_err(|e| format!("DB query_map: {}", e))?;
    
    let mut map = std::collections::HashMap::new();
    for row in iter.filter_map(|r| r.ok()) {
        map.insert(row.0, row.1);
    }
    Ok(map)
}

#[tauri::command]
pub async fn get_preset(id: String) -> Result<Option<String>, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare("SELECT config_json FROM user_presets WHERE id = ?1")
        .map_err(|e| format!("DB prepare: {}", e))?;
    let mut iter = stmt.query_map(params![id], |row| row.get(0))
        .map_err(|e| format!("DB query_map: {}", e))?;
    if let Some(Ok(val)) = iter.next() {
        Ok(Some(val))
    } else {
        Ok(None)
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn get_all_presets() -> Result<std::collections::HashMap<String, String>, String> {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare("SELECT id, config_json FROM user_presets")
        .map_err(|e| format!("DB prepare: {}", e))?;
    let iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })
    .map_err(|e| format!("DB query_map: {}", e))?;
    
    let mut map = std::collections::HashMap::new();
    for row in iter.filter_map(|r| r.ok()) {
        map.insert(row.0, row.1);
    }
    Ok(map)
}

#[tauri::command]
pub async fn save_preset(id: String, config_json: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO user_presets (id, config_json, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET config_json = excluded.config_json, updated_at = datetime('now')",
        params![id, config_json],
    )
    .map_err(|e| format!("DB execute: {}", e))?;
    Ok("Preset saved".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn delete_preset(id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute("DELETE FROM user_presets WHERE id = ?1", params![id])
        .map_err(|e| format!("DB delete: {}", e))?;
    Ok("Preset deleted".to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
