use reqwest::Client;
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn app_data_dir() -> PathBuf {
    let home = env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    PathBuf::from(home).join(".colima-ui")
}

fn read_language_from_db(db_path: &Path) -> Option<String> {
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

fn normalize_lang(raw: &str) -> String {
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

#[derive(Clone)]
struct I18n {
    lang: String,
}

impl I18n {
    fn new() -> Self {
        let db_path = app_data_dir().join("knowledge.db");
        let raw_lang = read_language_from_db(&db_path)
            .or_else(|| env::var("LANG").ok())
            .unwrap_or_else(|| "en".to_string());

        Self {
            lang: normalize_lang(&raw_lang),
        }
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
            ("vi", "err_port") => {
                "Không tìm thấy backend của Colima UI trên cổng 11420-11429. App có đang chạy không?"
            }

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

            // Default fallback (English)
            (_, "usage") => "Usage: cui <prompt...>",
            (_, "example") => "Example: cui Please list running containers",
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatCliOptions {
    prompt: String,
    allow_actions: bool,
    json_output: bool,
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    Empty,
    Help,
    Version,
    Mcp,
    Chat(ChatCliOptions),
}

fn help_text() -> String {
    format!(
        "cui {} — ColimaUI Headless AI & MCP CLI\n\n\
         USAGE:\n\
           cui \"<prompt>\"\n\
           cui --yes \"<prompt>\"\n\
           cui --json \"<prompt>\"\n\
           cui --provider <id> --model <name> \"<prompt>\"\n\
           cui mcp\n\n\
         SUBCOMMANDS:\n\
           mcp                  Start the Model Context Protocol (MCP) server over stdio\n\n\
         OPTIONS:\n\
           -y, --yes            Allow mutating DevOps actions (start/stop/restart/pull)\n\
               --json           Print raw JSON response from /api/cli/chat\n\
               --provider <id>  Override AI provider (e.g. ollama-local, openai, anthropic, gemini)\n\
               --model <name>   Override AI model name\n\
           -h, --help           Print help information\n\
           -V, --version        Print version information",
        env!("CARGO_PKG_VERSION")
    )
}

fn parse_cli_args(args: &[String]) -> Result<CliAction, String> {
    if args.is_empty() {
        return Ok(CliAction::Empty);
    }

    if args[0] == "mcp" {
        if args.len() == 1 {
            return Ok(CliAction::Mcp);
        }
        if args[1..].iter().any(|a| a == "-h" || a == "--help") {
            return Ok(CliAction::Help);
        }
        return Err(format!(
            "Unexpected arguments after 'mcp': {}",
            args[1..].join(" ")
        ));
    }

    let mut allow_actions = false;
    let mut json_output = false;
    let mut provider: Option<String> = None;
    let mut model: Option<String> = None;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut flags_done = false;
    let mut idx = 0usize;

    while idx < args.len() {
        let arg = &args[idx];
        if flags_done {
            prompt_parts.push(arg.clone());
            idx += 1;
            continue;
        }

        match arg.as_str() {
            "--" => {
                flags_done = true;
            }
            "-h" | "--help" if prompt_parts.is_empty() => {
                return Ok(CliAction::Help);
            }
            "-V" | "--version" if prompt_parts.is_empty() => {
                return Ok(CliAction::Version);
            }
            "-y" | "--yes" => {
                allow_actions = true;
            }
            "--json" => {
                json_output = true;
            }
            "--provider" => {
                idx += 1;
                let val = args
                    .get(idx)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('-'))
                    .ok_or_else(|| "Missing value for --provider <id>".to_string())?;
                provider = Some(val.to_string());
            }
            "--model" => {
                idx += 1;
                let val = args
                    .get(idx)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('-'))
                    .ok_or_else(|| "Missing value for --model <name>".to_string())?;
                model = Some(val.to_string());
            }
            _ if arg.starts_with("--provider=") => {
                let val = arg["--provider=".len()..].trim();
                if val.is_empty() {
                    return Err("Missing value for --provider=<id>".to_string());
                }
                provider = Some(val.to_string());
            }
            _ if arg.starts_with("--model=") => {
                let val = arg["--model=".len()..].trim();
                if val.is_empty() {
                    return Err("Missing value for --model=<name>".to_string());
                }
                model = Some(val.to_string());
            }
            _ if arg.starts_with('-') && prompt_parts.is_empty() => {
                return Err(format!("Unknown option '{arg}'. Run 'cui --help' for usage."));
            }
            _ => {
                prompt_parts.push(arg.clone());
            }
        }
        idx += 1;
    }

    let prompt = prompt_parts.join(" ").trim().to_string();
    if prompt.is_empty() {
        return Err("Missing <prompt>. Run 'cui --help' for usage.".to_string());
    }

    Ok(CliAction::Chat(ChatCliOptions {
        prompt,
        allow_actions,
        json_output,
        provider,
        model,
    }))
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
    let url = format!("http://127.0.0.1:{port}/api/health");
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
        if probe_health(client, port).await {
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
    fs::read_to_string(token_path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn extract_reply_text(json: &serde_json::Value) -> Option<&str> {
    json.get("reply")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("result").and_then(|v| v.as_str()))
        .or_else(|| json.pointer("/data/reply").and_then(|v| v.as_str()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let action = match parse_cli_args(&args) {
        Ok(act) => act,
        Err(err_msg) => {
            eprintln!("Error: {err_msg}");
            std::process::exit(1);
        }
    };

    match action {
        CliAction::Empty => {
            let i18n = I18n::new();
            println!("{}", i18n.t("usage"));
            println!("{}", i18n.t("example"));
        }
        CliAction::Help => {
            println!("{}", help_text());
        }
        CliAction::Version => {
            println!("cui {}", env!("CARGO_PKG_VERSION"));
        }
        CliAction::Mcp => {
            colima_ui_lib::shell_env::adopt();
            if let Err(err) = colima_ui_lib::mcp::run_stdio_server().await {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        CliAction::Chat(opts) => run_chat(opts).await?,
    }

    Ok(())
}

async fn run_chat(opts: ChatCliOptions) -> Result<(), Box<dyn std::error::Error>> {
    let i18n = I18n::new();
    let token = get_api_token();
    if token.is_empty() {
        eprintln!("{}", i18n.t("err_token"));
        eprintln!("{}", i18n.t("err_running"));
        std::process::exit(1);
    }

    if !opts.json_output {
        print!("{}", i18n.t("thinking"));
        io::stdout().flush()?;
    }

    let client = Client::new();
    let Some(port) = discover_port(&client).await else {
        if !opts.json_output {
            print!("\r\x1b[2K");
            io::stdout().flush()?;
        }
        eprintln!("{}", i18n.t("err_port"));
        std::process::exit(1);
    };

    let url = format!("http://127.0.0.1:{port}/api/cli/chat");
    let mut body = json!({
        "prompt": opts.prompt,
        "allow_actions": opts.allow_actions,
    });
    if let Some(provider) = opts.provider {
        body["provider"] = json!(provider);
    }
    if let Some(model) = opts.model {
        body["model"] = json!(model);
    }

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await;

    if !opts.json_output {
        print!("\r\x1b[2K");
        io::stdout().flush()?;
    }

    match response {
        Ok(res) => {
            let status = res.status();
            let text = res.text().await?;
            if opts.json_output {
                println!("{text}");
                if !status.is_success() {
                    std::process::exit(1);
                }
            } else if status.is_success() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(reply) = extract_reply_text(&parsed) {
                        println!("{reply}");
                    } else {
                        println!("{} {}", i18n.t("err_parse"), text);
                    }
                } else {
                    println!("{text}");
                }
            } else {
                eprintln!("{} ({status}): {text}", i18n.t("err_api"));
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}: {e}", i18n.t("err_conn"));
            eprintln!("{}", i18n.t("err_port"));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_string()).collect()
    }

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
            assert_ne!(
                ja.t(key),
                en.t(key),
                "ja translation for {key} fell back to en"
            );
        }
    }

    #[test]
    fn parse_cli_args_handles_subcommands_and_meta_flags() {
        assert_eq!(parse_cli_args(&to_args(&[])), Ok(CliAction::Empty));
        assert_eq!(parse_cli_args(&to_args(&["mcp"])), Ok(CliAction::Mcp));
        assert_eq!(parse_cli_args(&to_args(&["--help"])), Ok(CliAction::Help));
        assert_eq!(parse_cli_args(&to_args(&["-h"])), Ok(CliAction::Help));
        assert_eq!(
            parse_cli_args(&to_args(&["--version"])),
            Ok(CliAction::Version)
        );
        assert_eq!(parse_cli_args(&to_args(&["-V"])), Ok(CliAction::Version));
    }

    #[test]
    fn parse_cli_args_parses_chat_flags_and_prompt() {
        let parsed = parse_cli_args(&to_args(&[
            "--yes",
            "--json",
            "--provider",
            "anthropic",
            "--model",
            "claude-sonnet-4-6",
            "restart",
            "nginx",
        ]))
        .expect("should parse chat args");

        assert_eq!(
            parsed,
            CliAction::Chat(ChatCliOptions {
                prompt: "restart nginx".to_string(),
                allow_actions: true,
                json_output: true,
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4-6".to_string()),
            })
        );

        let eq_syntax = parse_cli_args(&to_args(&[
            "-y",
            "--provider=openai",
            "--model=gpt-4o-mini",
            "list containers",
        ]))
        .expect("should parse = syntax");

        assert_eq!(
            eq_syntax,
            CliAction::Chat(ChatCliOptions {
                prompt: "list containers".to_string(),
                allow_actions: true,
                json_output: false,
                provider: Some("openai".to_string()),
                model: Some("gpt-4o-mini".to_string()),
            })
        );
    }

    #[test]
    fn parse_cli_args_rejects_invalid_flags_or_missing_values() {
        assert!(parse_cli_args(&to_args(&["--unknown", "hello"])).is_err());
        assert!(parse_cli_args(&to_args(&["--provider"])).is_err());
        assert!(parse_cli_args(&to_args(&["--model", "--yes", "hi"])).is_err());
        assert!(parse_cli_args(&to_args(&["--yes"])).is_err());
        assert!(parse_cli_args(&to_args(&["mcp", "--extra"])).is_err());
    }

    #[test]
    fn read_language_from_db_reads_app_settings_table() {
        let tmp_path = std::env::temp_dir().join(format!(
            "cui-lang-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));

        let conn = Connection::open(&tmp_path).expect("open temp sqlite");
        conn.execute(
            "CREATE TABLE app_settings (setting_key TEXT PRIMARY KEY, setting_value TEXT NOT NULL)",
            [],
        )
        .expect("create app_settings");
        conn.execute(
            "INSERT INTO app_settings (setting_key, setting_value) VALUES ('language', 'vi')",
            [],
        )
        .expect("insert language");
        drop(conn);

        let detected = read_language_from_db(&tmp_path);
        let _ = fs::remove_file(&tmp_path);
        assert_eq!(detected.as_deref(), Some("vi"));
    }
}
