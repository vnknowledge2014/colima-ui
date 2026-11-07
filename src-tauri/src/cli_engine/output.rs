//! Output envelopes, UTF-8 safe table formatting, and localized `cui` messages.
//!
//! Guarantees zero spinner/ANSI noise on `stdout` whenever `--json` is active:
//! - Success: `{"ok": true, "data": ...}` (exit `0`)
//! - Failure: `{"ok": false, "error": "..."}` (exit `1`)

use std::env;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Value};

/// Read user language preference directly from `knowledge.db` without requiring initialization.
pub fn read_language_from_db(db_path: &Path) -> Option<String> {
    if !db_path.exists() {
        return None;
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT setting_value FROM app_settings WHERE setting_key IN ('language', 'app_language', 'app.language') LIMIT 1",
        )
        .ok()?;
    stmt.query_row([], |row| row.get::<_, String>(0))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn normalize_lang(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.starts_with("vi") {
        "vi".to_string()
    } else if lower.starts_with("zh") {
        "zh".to_string()
    } else if lower.starts_with("ja") {
        "ja".to_string()
    } else {
        "en".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct I18n {
    pub lang: String,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

impl I18n {
    pub fn new() -> Self {
        let db_path = crate::path_util::app_data_dir().join("knowledge.db");
        let raw_lang = read_language_from_db(&db_path)
            .or_else(|| env::var("LANG").ok())
            .unwrap_or_else(|| "en".to_string());

        Self {
            lang: normalize_lang(&raw_lang),
        }
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        match (self.lang.as_str(), key) {
            ("vi", "usage") => "Sử dụng: cui <lệnh | câu hỏi...>",
            ("vi", "example") => "Ví dụ: cui Hãy liệt kê danh sách các container đang chạy",
            ("vi", "err_token") => "Lỗi: Không thể đọc API token từ ~/.colima-ui/api_token.",
            ("vi", "err_running") => "Vui lòng đảm bảo Colima UI đang chạy ở chế độ nền.",
            ("vi", "thinking") => "Đang suy nghĩ... ",
            ("vi", "err_parse") => "Thành công, nhưng không thể phân tích kết quả:",
            ("vi", "err_api") => "Lỗi API",
            ("vi", "err_conn") => "Lỗi kết nối",
            ("vi", "err_port") => {
                "Không tìm thấy backend của Colima UI trên cổng 11420-11429. App có đang chạy không?"
            }

            ("zh", "usage") => "用法: cui <子命令 | 提示词...>",
            ("zh", "example") => "示例: cui 请列出正在运行的容器",
            ("zh", "err_token") => "错误: 无法读取 ~/.colima-ui/api_token。",
            ("zh", "err_running") => "请确保 Colima UI 正在后台运行。",
            ("zh", "thinking") => "思考中... ",
            ("zh", "err_parse") => "成功，但无法解析结果:",
            ("zh", "err_api") => "API 错误",
            ("zh", "err_conn") => "连接错误",
            ("zh", "err_port") => "在端口 11420-11429 上找不到 Colima UI 后端。应用正在运行吗?",

            ("ja", "usage") => "使い方: cui <サブコマンド | プロンプト...>",
            ("ja", "example") => "例: cui 実行中のコンテナ一覧を表示して",
            ("ja", "err_token") => {
                "エラー: ~/.colima-ui/api_token から API トークンを読み取れませんでした。"
            }
            ("ja", "err_running") => {
                "Colima UI がバックグラウンドで実行されていることを確認してください。"
            }
            ("ja", "thinking") => "思考中... ",
            ("ja", "err_parse") => "成功しましたが、結果を解析できませんでした:",
            ("ja", "err_api") => "API エラー",
            ("ja", "err_conn") => "接続エラー",
            ("ja", "err_port") => {
                "ポート 11420-11429 で応答する Colima UI バックエンドが見つかりません。アプリは起動していますか?"
            }

            (_, "usage") => "Usage: cui <command | prompt...>",
            (_, "example") => "Example: cui Please list running containers  (or `cui --help`)",
            (_, "err_token") => "Error: Could not read API token from ~/.colima-ui/api_token.",
            (_, "err_running") => "Please ensure Colima UI is running in the background.",
            (_, "thinking") => "Thinking... ",
            (_, "err_parse") => "Success, but could not parse result:",
            (_, "err_api") => "API Error",
            (_, "err_conn") => "Connection Error",
            (_, "err_port") => {
                "No Colima UI backend answered on ports 11420-11429. Is the app running?"
            }

            (_, _) => key,
        }
    }
}

/// UTF-8 character-safe truncation (`…` suffix) that never panics on Vietnamese, CJK, or 4-byte emoji.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() > max {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

/// Human-friendly byte formatter (`GiB` / `MiB`).
pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "-".to_string();
    }
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{gb:.1}GiB")
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{mb:.0}MiB")
    }
}

/// Print `{"ok": true, "data": ...}` cleanly to `stdout`.
pub fn print_json_ok<T: Serialize>(data: &T) {
    let envelope = json!({
        "ok": true,
        "data": data,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| r#"{"ok":true,"data":null}"#.to_string())
    );
}

/// Print `{"ok": false, "error": "..."}` cleanly to `stdout`.
pub fn print_json_err(err: &str) {
    let envelope = json!({
        "ok": false,
        "error": err,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope)
            .unwrap_or_else(|_| format!(r#"{{"ok":false,"error":{err:?}}}"#))
    );
}

/// Extract `[SUGGEST: ...]` follow-up prompts from an AI reply, returning `(clean_reply, suggestions)`.
pub fn extract_suggestions(raw_reply: &str) -> (String, Vec<String>) {
    let mut suggestions = Vec::new();
    let mut clean_out = String::with_capacity(raw_reply.len());
    let mut cursor = 0usize;

    while let Some(rel_open) = raw_reply[cursor..].find('[') {
        let open_idx = cursor + rel_open;
        let rest = &raw_reply[open_idx..];
        let prefix = "[SUGGEST:";
        if rest
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            clean_out.push_str(&raw_reply[cursor..open_idx]);
            let inner_start = open_idx + prefix.len();
            if let Some(rel_close) = raw_reply[inner_start..].find(']') {
                let item = raw_reply[inner_start..inner_start + rel_close].trim();
                if !item.is_empty() {
                    suggestions.push(item.to_string());
                }
                cursor = inner_start + rel_close + 1;
                continue;
            }
        }
        clean_out.push_str(&raw_reply[cursor..=open_idx]);
        cursor = open_idx + 1;
    }

    if cursor < raw_reply.len() {
        clean_out.push_str(&raw_reply[cursor..]);
    }

    (clean_out.trim().to_string(), suggestions)
}

/// Render a JSON value in human-readable form when no custom table applies.
pub fn print_human_value(val: &Value) {
    match val {
        Value::Null => println!("OK"),
        Value::String(s) => {
            if s.ends_with('\n') {
                print!("{s}");
            } else {
                println!("{s}");
            }
        }
        Value::Bool(b) => println!("{b}"),
        Value::Number(n) => println!("{n}"),
        other => {
            println!(
                "{}",
                serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())
            );
        }
    }
}
