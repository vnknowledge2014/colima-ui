use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// ===== Global DB Connection =====

static DB: std::sync::OnceLock<Mutex<Connection>> = std::sync::OnceLock::new();

fn db() -> &'static Mutex<Connection> {
    DB.get().expect("Knowledge bank not initialized. Call init_knowledge_bank() first.")
}

/// Initialize the knowledge bank — called once from lib.rs setup()
pub fn init_knowledge_bank() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let dir = format!("{}/.colima-ui", home);
    let _ = std::fs::create_dir_all(&dir);
    let db_path = format!("{}/knowledge.db", dir);

    let conn = Connection::open(&db_path).expect("Failed to open knowledge.db");

    // Create tables
    conn.execute_batch("
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
    ").expect("Failed to create knowledge bank tables");

    // Seed builtin solutions (only if empty)
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM solutions WHERE source = 'builtin'", [], |r| r.get(0)
    ).unwrap_or(0);

    if count == 0 {
        seed_builtins(&conn);
    }

    DB.set(Mutex::new(conn)).expect("Knowledge bank already initialized");
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
    pub context_text: String,  // Pre-formatted for AI injection
}

// ===== Tauri Commands =====

/// Query knowledge bank for matching solutions + anti-patterns
#[tauri::command]
pub async fn kb_query(error_text: String) -> Result<KBQueryResult, String> {
    let error_lower = error_text.to_lowercase();
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;

    // Find matching solutions (simple keyword matching from error_pattern)
    let mut stmt = conn.prepare(
        "SELECT id, error_pattern, error_category, solution_text, root_cause, commands, likes, dislikes, source
         FROM solutions ORDER BY (likes - dislikes) DESC, likes DESC"
    ).map_err(|e| format!("DB query: {}", e))?;

    let all_solutions: Vec<KBMatch> = stmt.query_map([], |row| {
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
    }).map_err(|e| format!("DB map: {}", e))?
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
    let mut ap_stmt = conn.prepare(
        "SELECT id, error_pattern, bad_suggestion, reason FROM anti_patterns"
    ).map_err(|e| format!("DB query: {}", e))?;

    let all_aps: Vec<KBAntiPattern> = ap_stmt.query_map([], |row| {
        Ok(KBAntiPattern {
            id: row.get(0)?,
            error_pattern: row.get(1)?,
            bad_suggestion: row.get(2)?,
            reason: row.get(3)?,
        })
    }).map_err(|e| format!("DB map: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let anti_patterns: Vec<KBAntiPattern> = all_aps.into_iter()
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
            let badge = if sol.source == "learned" { "🧠 Learned" } else { "📖 Built-in" };
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
            params![sol.id]
        );
    }

    Ok(KBQueryResult { solutions, anti_patterns, context_text: context })
}

/// Record feedback (like/dislike) for a solution
#[tauri::command]
pub async fn kb_feedback(solution_id: i64, is_like: bool) -> Result<String, String> {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    let column = if is_like { "likes" } else { "dislikes" };
    conn.execute(
        &format!("UPDATE solutions SET {} = {} + 1 WHERE id = ?1", column, column),
        params![solution_id]
    ).map_err(|e| format!("DB update: {}", e))?;
    Ok(if is_like { "Solution liked".to_string() } else { "Solution disliked".to_string() })
}

/// Save a new learned solution from AI response
#[tauri::command]
pub async fn kb_save_solution(
    error_pattern: String,
    error_category: String,
    solution_text: String,
    root_cause: String,
) -> Result<i64, String> {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO solutions (error_pattern, error_category, solution_text, root_cause, commands, source, likes)
         VALUES (?1, ?2, ?3, ?4, '[]', 'learned', 1)",
        params![error_pattern, error_category, solution_text, root_cause]
    ).map_err(|e| format!("DB insert: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Save an anti-pattern (approach that didn't work)
#[tauri::command]
pub async fn kb_save_anti_pattern(
    error_pattern: String,
    bad_suggestion: String,
    reason: String,
) -> Result<i64, String> {
    let conn = db().lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO anti_patterns (error_pattern, bad_suggestion, reason) VALUES (?1, ?2, ?3)",
        params![error_pattern, bad_suggestion, reason]
    ).map_err(|e| format!("DB insert: {}", e))?;
    Ok(conn.last_insert_rowid())
}
