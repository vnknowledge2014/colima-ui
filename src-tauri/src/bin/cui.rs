use reqwest::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::{self, Write};
use rusqlite::{Connection, Result as SqlResult};

fn app_data_dir() -> PathBuf {
    let home = env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    PathBuf::from(home).join(".colima-ui")
}

#[derive(Clone)]
struct I18n {
    lang: String,
}

impl I18n {
    fn new() -> Self {
        let mut lang = env::var("LANG").unwrap_or_else(|_| "en".to_string());

        // Try reading from SQLite knowledge.db
        let db_path = app_data_dir().join("knowledge.db");

        if db_path.exists() {
            if let Ok(conn) = Connection::open(&db_path) {
                let stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1");
                if let Ok(mut stmt) = stmt {
                    let val: SqlResult<String> = stmt.query_row(["app.language"], |row| row.get(0));
                    if let Ok(v) = val {
                        lang = v;
                    }
                }
            }
        }

        // Normalize language
        if lang.starts_with("vi") {
            lang = "vi".to_string();
        } else if lang.starts_with("zh") {
            lang = "zh".to_string();
        } else if lang.starts_with("ja") {
            lang = "ja".to_string();
        } else {
            lang = "en".to_string();
        }

        Self { lang }
    }

    fn t<'a>(&self, key: &'a str) -> &'a str {
        match (self.lang.as_str(), key) {
            ("vi", "usage") => "Sử dụng: cui <lệnh...>",
            ("vi", "example") => "Ví dụ: cui Hãy liệt kê danh sách các container đang chạy",
            ("vi", "err_token") => "Lỗi: Không thể đọc API token từ ~/.colima-ui/api_token.",
            ("vi", "err_running") => "Vui lòng đảm bảo Colima UI đang chạy ở chế độ nền.",
            ("vi", "thinking") => "Đang suy nghĩ... ",
            ("vi", "err_parse") => "Thành công, nhưng không thể phân tích kết quả:",
            ("vi", "err_api") => "Lỗi API",
            ("vi", "err_conn") => "Lỗi kết nối",
            ("vi", "err_port") => "Không tìm thấy backend của Colima UI trên cổng 11420-11429. App có đang chạy không?",

            ("zh", "usage") => "用法: cui <提示词...>",
            ("zh", "example") => "示例: cui 请列出正在运行的容器",
            ("zh", "err_token") => "错误: 无法读取 ~/.colima-ui/api_token。",
            ("zh", "err_running") => "请确保 Colima UI 正在后台运行。",
            ("zh", "thinking") => "思考中... ",
            ("zh", "err_parse") => "成功，但无法解析结果:",
            ("zh", "err_api") => "API 错误",
            ("zh", "err_conn") => "连接错误",
            ("zh", "err_port") => "在端口 11420-11429 上找不到 Colima UI 后端。应用正在运行吗?",

            ("ja", "usage") => "使い方: cui <プロンプト...>",
            ("ja", "example") => "例: cui 実行中のコンテナ一覧を表示して",
            ("ja", "err_token") => "エラー: ~/.colima-ui/api_token から API トークンを読み取れませんでした。",
            ("ja", "err_running") => "Colima UI がバックグラウンドで実行されていることを確認してください。",
            ("ja", "thinking") => "思考中... ",
            ("ja", "err_parse") => "成功しましたが、結果を解析できませんでした:",
            ("ja", "err_api") => "API エラー",
            ("ja", "err_conn") => "接続エラー",
            ("ja", "err_port") => "ポート 11420-11429 で応答する Colima UI バックエンドが見つかりません。アプリは起動していますか?",

            // Default fallback (English)
            (_, "usage") => "Usage: cui <prompt...>",
            (_, "example") => "Example: cui Please list running containers",
            (_, "err_token") => "Error: Could not read API token from ~/.colima-ui/api_token.",
            (_, "err_running") => "Please ensure Colima UI is running in the background.",
            (_, "thinking") => "Thinking... ",
            (_, "err_parse") => "Success, but could not parse result:",
            (_, "err_api") => "API Error",
            (_, "err_conn") => "Connection Error",
            (_, "err_port") => "No Colima UI backend answered on ports 11420-11429. Is the app running?",

            (_, _) => key,
        }
    }
}

/// The range the app binds in, and the order it tries.
///
/// `start_api_server` takes the first free port here, so a second instance —
/// or a stale one holding 11420 — pushes the live server up the range. This
/// binary used to assume 11420 and simply fail when the app was anywhere else,
/// reporting "is the backend running?" while it plainly was.
const PORT_RANGE: std::ops::RangeInclusive<u16> = 11420..=11429;

/// Parse a port string and accept it only when it lies inside [`PORT_RANGE`].
fn parse_recorded_port(raw: &str) -> Option<u16> {
    raw.trim()
        .parse::<u16>()
        .ok()
        .filter(|port| PORT_RANGE.contains(port))
}

/// Read the active session's bound port recorded in `~/.colima-ui/api_port`.
fn read_recorded_port() -> Option<u16> {
    let port_path = app_data_dir().join("api_port");
    let raw = fs::read_to_string(port_path).ok()?;
    parse_recorded_port(&raw)
}

async fn probe_health(client: &Client, port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/api/health", port);
    // Short timeout: a closed local port fails immediately, but a filtered
    // one would otherwise hang the CLI for the OS default.
    client
        .get(&url)
        .timeout(std::time::Duration::from_millis(300))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

/// Find the port the active app instance is serving on.
///
/// Probes `~/.colima-ui/api_port` first via `/api/health` so `cui` reaches the
/// instance that wrote `~/.colima-ui/api_token` even when a stale process still
/// occupies port 11420, then falls back to scanning `PORT_RANGE`.
async fn discover_port(client: &Client) -> Option<u16> {
    let recorded = read_recorded_port();
    if let Some(port) = recorded {
        let healthy = probe_health(client, port).await;
        if healthy {
            return Some(port);
        }
    }
    for port in PORT_RANGE {
        if recorded == Some(port) {
            continue;
        }
        if probe_health(client, port).await {
            return Some(port);
        }
    }
    None
}

fn get_api_token() -> String {
    let token_path = app_data_dir().join("api_token");
    fs::read_to_string(&token_path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let i18n = I18n::new();
    
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        println!("{}", i18n.t("usage"));
        println!("{}", i18n.t("example"));
        return Ok(());
    }
    
    let prompt = args.join(" ");
    let token = get_api_token();
    if token.is_empty() {
        eprintln!("{}", i18n.t("err_token"));
        eprintln!("{}", i18n.t("err_running"));
        std::process::exit(1);
    }
    
    print!("{}", i18n.t("thinking"));
    io::stdout().flush()?;

    let client = Client::new();
    let Some(port) = discover_port(&client).await else {
        // Clear the "Thinking… " line before saying nothing was found.
        print!("\r\x1b[2K");
        io::stdout().flush()?;
        eprintln!("{}", i18n.t("err_port"));
        std::process::exit(1);
    };
    let url = format!("http://127.0.0.1:{}/api/cli/chat", port);
    let body = json!({
        "prompt": prompt
    });
    
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await;
        
    // Clear the "Thinking... " text
    print!("\r\x1b[2K");
    io::stdout().flush()?;
        
    match response {
        Ok(res) => {
            let status = res.status();
            let text = res.text().await?;
            if status.is_success() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(result) = json.get("result").and_then(|v| v.as_str()) {
                        println!("{}", result);
                    } else {
                        println!("{} {}", i18n.t("err_parse"), text);
                    }
                } else {
                    println!("{}", text);
                }
            } else {
                eprintln!("{} ({}): {}", i18n.t("err_api"), status, text);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{}: {}", i18n.t("err_conn"), e);
            eprintln!("{}", i18n.t("err_port"));
            std::process::exit(1);
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recorded_port_accepts_in_range_and_rejects_invalid() {
        assert_eq!(parse_recorded_port("11420"), Some(11420));
        assert_eq!(parse_recorded_port(" 11425\n"), Some(11425));
        assert_eq!(parse_recorded_port("11429"), Some(11429));
        assert_eq!(parse_recorded_port("11419"), None);
        assert_eq!(parse_recorded_port("11430"), None);
        assert_eq!(parse_recorded_port(""), None);
        assert_eq!(parse_recorded_port("not-a-port"), None);
    }

    #[test]
    fn japanese_locale_covers_all_cli_messages() {
        let ja = I18n {
            lang: "ja".to_string(),
        };
        let en = I18n {
            lang: "en".to_string(),
        };
        for key in [
            "usage",
            "example",
            "err_token",
            "err_running",
            "thinking",
            "err_parse",
            "err_api",
            "err_conn",
            "err_port",
        ] {
            assert_ne!(ja.t(key), key, "missing ja translation for {key}");
            assert_ne!(ja.t(key), en.t(key), "ja translation for {key} fell back to en");
        }
    }
}
