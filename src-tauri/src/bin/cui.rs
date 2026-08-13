use reqwest::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::{self, Write};
use rusqlite::{Connection, Result as SqlResult};

#[derive(Clone)]
struct I18n {
    lang: String,
}

impl I18n {
    fn new() -> Self {
        let mut lang = env::var("LANG").unwrap_or_else(|_| "en".to_string());
        
        // Try reading from SQLite knowledge.db
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let db_path = PathBuf::from(&home).join(".colima-ui").join("knowledge.db");
        
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

/// Find the port the app is actually serving on.
///
/// `/api/health` is unauthenticated precisely so a client can do this before it
/// has a credential — the webview already scans the same way. Returns the first
/// port that answers.
///
/// What this does **not** solve: with two instances running, the lowest port
/// wins, and that may be the older one. Choosing between live instances needs
/// the app to publish which one is current; see the TODO in `api_server.rs`.
async fn discover_port(client: &Client) -> Option<u16> {
    for port in PORT_RANGE {
        let url = format!("http://127.0.0.1:{}/api/health", port);
        // Short timeout: a closed local port fails immediately, but a filtered
        // one would otherwise hang the CLI for the OS default.
        let probe = client
            .get(&url)
            .timeout(std::time::Duration::from_millis(300))
            .send()
            .await;
        if probe.is_ok_and(|r| r.status().is_success()) {
            return Some(port);
        }
    }
    None
}

fn get_api_token() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let token_path = PathBuf::from(home).join(".colima-ui").join("api_token");
    fs::read_to_string(&token_path)
        .unwrap_or_else(|_| "".to_string())
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
