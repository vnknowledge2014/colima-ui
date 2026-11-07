//! Unified CLI-First Engine (`v0.3.0`) for `cui` and `colimaui`.
//!
//! Architecture:
//! - Clap v4 command tree (`args`) covering all 22 DevOps, Security, Knowledge Bank,
//!   Sessions, UI, Events, Doctor, Chat, and MCP domains plus legacy `system` / `status`.
//! - Global `--json` support in any argument position (`{"ok": true, "data": ...}` /
//!   `{"ok": false, "error": "..."}`) with zero ANSI or spinner output on `stdout`.
//! - Backward-compatible free-form shorthand on `cui` (`cui "Check Colima status"` or
//!   `cui --yes "Restart redis"` routes seamlessly to `Commands::Chat` on the isolated
//!   `colima-a2a-default-session` thread).
//! - Dual-transport execution (`HttpOrDirect`): `< 5ms` local REST fast-path when
//!   ColimaUI Desktop is open, falling back to direct in-process `colima_ui_lib` calls
//!   when Desktop is closed.

pub mod args;
pub mod client;
pub(crate) mod handlers_infra;
pub(crate) mod handlers_ops;
pub mod output;

use std::path::Path;
use std::sync::Once;

use clap::{CommandFactory, FromArgMatches};

use self::args::{ChatArgs, Cli, Commands, DEFAULT_A2A_SESSION_ID, KNOWN_TOP_SUBCOMMANDS};
use self::client::DesktopClient;
use self::output::{print_json_err, I18n};

static ENV_INIT: Once = Once::new();
static KB_INIT: Once = Once::new();

/// Adopt login shell environment and PATH exactly once per process.
/// Should be called from synchronous `fn main()` before starting the Tokio runtime.
pub fn init_process_env() {
    ENV_INIT.call_once(|| {
        crate::shell_env::adopt();
        crate::path_util::fix_path_env();
    });
}

/// Lazily initialize `~/.colima-ui/knowledge.db` once in standalone CLI / MCP mode.
pub fn ensure_kb_init() {
    KB_INIT.call_once(|| {
        let _ = std::panic::catch_unwind(crate::commands::knowledge_bank::init_knowledge_bank);
    });
}

/// Resolved execution plan produced from raw command-line arguments.
#[derive(Debug)]
pub enum InvocationPlan {
    /// `cui` invoked with zero arguments — display localized usage hint and exit `0`.
    EmptyCui,
    /// Display `--help` or `--version` text on `stdout` and exit `0`.
    DisplayMeta(String),
    /// Execute a parsed command with `use_json` envelope mode.
    Execute { use_json: bool, command: Commands },
}

/// Early CLI argument error prior to handler execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliParseError {
    pub message: String,
    pub json_requested: bool,
}

fn is_cui_binary(bin_name: &str) -> bool {
    Path::new(bin_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("cui"))
}

/// Detect whether `--json` or `--format json` / `--format=json` appears anywhere before `--`.
pub fn detect_json_flag(raw_args: &[String]) -> bool {
    let mut idx = 0usize;
    while idx < raw_args.len() {
        let arg = raw_args[idx].as_str();
        if arg == "--" {
            break;
        }
        if arg == "--json" {
            return true;
        }
        if arg.strip_prefix("--format=").is_some_and(|val| val.trim().eq_ignore_ascii_case("json")) {
            return true;
        }
        if arg == "--format" {
            if raw_args
                .get(idx + 1)
                .is_some_and(|next| next.trim().eq_ignore_ascii_case("json"))
            {
                return true;
            }
            idx += 2;
            continue;
        }
        idx += 1;
    }
    false
}

fn take_option_value(
    raw_args: &[String],
    idx: &mut usize,
    flag: &str,
    json_requested: bool,
) -> Result<String, CliParseError> {
    let val = raw_args.get(*idx + 1).ok_or_else(|| CliParseError {
        message: format!("Option '{flag}' requires a value"),
        json_requested,
    })?;
    if val.trim().is_empty() || val.starts_with('-') {
        return Err(CliParseError {
            message: format!("Option '{flag}' requires a non-empty value"),
            json_requested,
        });
    }
    *idx += 2;
    Ok(val.trim().to_string())
}

fn take_inline_value(
    inline: &str,
    flag: &str,
    json_requested: bool,
) -> Result<String, CliParseError> {
    let trimmed = inline.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return Err(CliParseError {
            message: format!("Option '{flag}' requires a non-empty value"),
            json_requested,
        });
    }
    Ok(trimmed.to_string())
}

fn parse_with_clap(
    bin_name: &str,
    raw_args: &[String],
    json_requested: bool,
) -> Result<InvocationPlan, CliParseError> {
    let mut full = Vec::with_capacity(raw_args.len() + 1);
    full.push(bin_name.to_string());
    full.extend(raw_args.iter().cloned());

    let static_name: &'static str = if is_cui_binary(bin_name) {
        "cui"
    } else {
        "colimaui"
    };
    let cmd = Cli::command()
        .name(static_name)
        .bin_name(bin_name.to_string());
    match cmd.try_get_matches_from(full) {
        Ok(matches) => match Cli::from_arg_matches(&matches) {
            Ok(cli) => Ok(InvocationPlan::Execute {
                use_json: cli.use_json() || json_requested,
                command: cli.command,
            }),
            Err(err) => Err(CliParseError {
                message: err.to_string().trim().to_string(),
                json_requested,
            }),
        },
        Err(err) => match err.kind() {
            clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayVersion
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                Ok(InvocationPlan::DisplayMeta(err.to_string()))
            }
            _ => Err(CliParseError {
                message: err.to_string().trim().to_string(),
                json_requested,
            }),
        },
    }
}

/// Resolve raw CLI arguments (excluding `argv[0]`) into an [`InvocationPlan`].
///
/// When `bin_name` is `cui`, supports both structured subcommands (`cui vm list`,
/// `cui --json doctor`, `cui mcp`) and natural-language shorthand
/// (`cui "Check Colima status"`, `cui --yes "Restart redis"`).
pub fn resolve_invocation(
    bin_name: &str,
    raw_args: &[String],
) -> Result<InvocationPlan, CliParseError> {
    let json_requested = detect_json_flag(raw_args);

    if !is_cui_binary(bin_name) {
        return parse_with_clap(bin_name, raw_args, json_requested);
    }

    if raw_args.is_empty() {
        return Ok(InvocationPlan::EmptyCui);
    }

    let mut yes = false;
    let mut use_json = json_requested;
    let mut mirror_ui = false;
    let mut session = DEFAULT_A2A_SESSION_ID.to_string();
    let mut provider: Option<String> = None;
    let mut model: Option<String> = None;
    let mut chat_only_flag_used = false;
    let mut flags_done = false;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut idx = 0usize;

    while idx < raw_args.len() {
        let arg = raw_args[idx].as_str();

        if flags_done {
            prompt_parts.push(arg.to_string());
            idx += 1;
            continue;
        }

        // Before the first positional word is encountered, check whether this invocation
        // is a meta flag (`-h`/`--help`/`-V`/`--version`), a known subcommand (`vm`, `doctor`, `mcp`),
        // or a leading chat/global flag.
        if prompt_parts.is_empty() {
            match arg {
                "--" => {
                    flags_done = true;
                    idx += 1;
                    continue;
                }
                "-h" | "--help" | "-V" | "--version" => {
                    return parse_with_clap(bin_name, raw_args, json_requested);
                }
                "-y" | "--yes" => {
                    yes = true;
                    chat_only_flag_used = true;
                    idx += 1;
                    continue;
                }
                "--mirror-ui" => {
                    mirror_ui = true;
                    chat_only_flag_used = true;
                    idx += 1;
                    continue;
                }
                "--json" => {
                    use_json = true;
                    idx += 1;
                    continue;
                }
                "--format" => {
                    let fmt = take_option_value(raw_args, &mut idx, "--format", json_requested)?;
                    if fmt.eq_ignore_ascii_case("json") {
                        use_json = true;
                    }
                    continue;
                }
                "--session" => {
                    session = take_option_value(raw_args, &mut idx, "--session", json_requested)?;
                    chat_only_flag_used = true;
                    continue;
                }
                "--provider" => {
                    provider = Some(take_option_value(
                        raw_args,
                        &mut idx,
                        "--provider",
                        json_requested,
                    )?);
                    chat_only_flag_used = true;
                    continue;
                }
                "--model" => {
                    model = Some(take_option_value(
                        raw_args,
                        &mut idx,
                        "--model",
                        json_requested,
                    )?);
                    chat_only_flag_used = true;
                    continue;
                }
                _ => {
                    if let Some(v) = arg.strip_prefix("--format=") {
                        let fmt = take_inline_value(v, "--format", json_requested)?;
                        if fmt.eq_ignore_ascii_case("json") {
                            use_json = true;
                        }
                        idx += 1;
                        continue;
                    }
                    if let Some(v) = arg.strip_prefix("--session=") {
                        session = take_inline_value(v, "--session", json_requested)?;
                        chat_only_flag_used = true;
                        idx += 1;
                        continue;
                    }
                    if let Some(v) = arg.strip_prefix("--provider=") {
                        provider = Some(take_inline_value(v, "--provider", json_requested)?);
                        chat_only_flag_used = true;
                        idx += 1;
                        continue;
                    }
                    if let Some(v) = arg.strip_prefix("--model=") {
                        model = Some(take_inline_value(v, "--model", json_requested)?);
                        chat_only_flag_used = true;
                        idx += 1;
                        continue;
                    }
                    if arg.starts_with('-') {
                        return Err(CliParseError {
                            message: format!(
                                "Unknown option '{arg}'. Run '{bin_name} --help' for usage."
                            ),
                            json_requested,
                        });
                    }

                    // First positional token! If it matches a known top-level subcommand (and no
                    // chat-only shorthand flags preceded it, or let Clap validate subcommand args),
                    // delegate the entire invocation to Clap.
                    if KNOWN_TOP_SUBCOMMANDS.contains(&arg) {
                        if chat_only_flag_used && arg != "chat" {
                            return parse_with_clap(bin_name, raw_args, json_requested);
                        }
                        return parse_with_clap(bin_name, raw_args, json_requested);
                    }

                    prompt_parts.push(arg.to_string());
                    idx += 1;
                    continue;
                }
            }
        }

        // Subsequent tokens in free-form `cui` chat shorthand: honor global/chat flags
        // anywhere before `--` while preserving arbitrary prompt words.
        match arg {
            "--" => {
                flags_done = true;
                idx += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                idx += 1;
            }
            "--mirror-ui" => {
                mirror_ui = true;
                idx += 1;
            }
            "--json" => {
                use_json = true;
                idx += 1;
            }
            "--format" => {
                let fmt = take_option_value(raw_args, &mut idx, "--format", json_requested)?;
                if fmt.eq_ignore_ascii_case("json") {
                    use_json = true;
                }
            }
            "--session" => {
                session = take_option_value(raw_args, &mut idx, "--session", json_requested)?;
            }
            "--provider" => {
                provider = Some(take_option_value(
                    raw_args,
                    &mut idx,
                    "--provider",
                    json_requested,
                )?);
            }
            "--model" => {
                model = Some(take_option_value(
                    raw_args,
                    &mut idx,
                    "--model",
                    json_requested,
                )?);
            }
            _ => {
                if let Some(v) = arg.strip_prefix("--format=") {
                    let fmt = take_inline_value(v, "--format", json_requested)?;
                    if fmt.eq_ignore_ascii_case("json") {
                        use_json = true;
                    }
                    idx += 1;
                } else if let Some(v) = arg.strip_prefix("--session=") {
                    session = take_inline_value(v, "--session", json_requested)?;
                    idx += 1;
                } else if let Some(v) = arg.strip_prefix("--provider=") {
                    provider = Some(take_inline_value(v, "--provider", json_requested)?);
                    idx += 1;
                } else if let Some(v) = arg.strip_prefix("--model=") {
                    model = Some(take_inline_value(v, "--model", json_requested)?);
                    idx += 1;
                } else {
                    prompt_parts.push(arg.to_string());
                    idx += 1;
                }
            }
        }
    }

    if prompt_parts.join(" ").trim().is_empty() {
        return Err(CliParseError {
            message: format!("Missing prompt or subcommand. Run '{bin_name} --help' for usage."),
            json_requested: use_json,
        });
    }

    Ok(InvocationPlan::Execute {
        use_json,
        command: Commands::Chat(ChatArgs {
            message: prompt_parts,
            session,
            yes,
            mirror_ui,
            provider,
            model,
        }),
    })
}

/// Unified async entrypoint called by both `src/bin/cui.rs` and `src/bin/cli.rs`.
/// Returns process exit code (`0` on success, `1` on error).
pub async fn run_from_env(bin_name: &str) -> i32 {
    init_process_env();

    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let plan = match resolve_invocation(bin_name, &raw_args) {
        Ok(p) => p,
        Err(err) => {
            if err.json_requested {
                print_json_err(&err.message);
            } else {
                eprintln!("{}", err.message);
            }
            return 1;
        }
    };

    match plan {
        InvocationPlan::EmptyCui => {
            let i18n = I18n::new();
            println!("{}", i18n.t("usage"));
            println!("{}", i18n.t("example"));
            0
        }
        InvocationPlan::DisplayMeta(text) => {
            print!("{text}");
            if !text.ends_with('\n') {
                println!();
            }
            0
        }
        InvocationPlan::Execute { use_json, command } => {
            let result = execute_command(command, use_json).await;
            match result {
                Ok(()) => 0,
                Err(err_msg) => {
                    if use_json {
                        print_json_err(&err_msg);
                    } else {
                        eprintln!("Error: {err_msg}");
                    }
                    1
                }
            }
        }
    }
}

async fn execute_command(command: Commands, use_json: bool) -> Result<(), String> {
    if matches!(command, Commands::Mcp) {
        return handlers_ops::handle_mcp().await;
    }

    let desktop_client = DesktopClient::discover().await;
    let client_ref = desktop_client.as_ref();

    match command {
        Commands::Vm { action } => handlers_infra::handle_vm(action, client_ref, use_json).await,
        Commands::Config { action } => {
            handlers_infra::handle_config(action, client_ref, use_json).await
        }
        Commands::Presets { action } => {
            handlers_infra::handle_presets(action, client_ref, use_json).await
        }
        Commands::Container { action } => {
            handlers_infra::handle_container(action, client_ref, use_json).await
        }
        Commands::Image { action } => {
            handlers_infra::handle_image(action, client_ref, use_json).await
        }
        Commands::Volume { action } => {
            handlers_infra::handle_volume(action, client_ref, use_json).await
        }
        Commands::Network { action } => {
            handlers_infra::handle_network(action, client_ref, use_json).await
        }
        Commands::Compose { action } => {
            handlers_infra::handle_compose(action, client_ref, use_json).await
        }
        Commands::K8s { action } => handlers_infra::handle_k8s(action, client_ref, use_json).await,
        Commands::Kind { action } => {
            handlers_infra::handle_kind(action, client_ref, use_json).await
        }
        Commands::Lima { action } => {
            handlers_infra::handle_lima(action, client_ref, use_json).await
        }
        Commands::Security { action } => {
            handlers_ops::handle_security(action, client_ref, use_json).await
        }
        Commands::Models { action } => {
            handlers_ops::handle_models(action, client_ref, use_json).await
        }
        Commands::Alerts { action } => {
            handlers_ops::handle_alerts(action, client_ref, use_json).await
        }
        Commands::SelfHeal { action } => {
            handlers_ops::handle_self_heal(action, client_ref, use_json).await
        }
        Commands::Memories { action } => {
            handlers_ops::handle_memories(action, client_ref, use_json).await
        }
        Commands::Sessions { action } => {
            handlers_ops::handle_sessions(action, client_ref, use_json).await
        }
        Commands::Ui { action } => handlers_ops::handle_ui(action, client_ref, use_json).await,
        Commands::Events(args) => handlers_ops::handle_events(args, client_ref, use_json).await,
        Commands::Doctor => handlers_ops::handle_doctor(client_ref, use_json).await,
        Commands::Chat(args) => handlers_ops::handle_chat(args, client_ref, use_json).await,
        Commands::Mcp => handlers_ops::handle_mcp().await,
        Commands::System { action } => {
            handlers_infra::handle_system(action, client_ref, use_json).await
        }
        Commands::Status => handlers_infra::handle_status(client_ref, use_json).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_engine::args::{ContainerCommand, VmCommand};
    use crate::cli_engine::client::{parse_recorded_port, unwrap_response_json, HttpOutcome};
    use crate::cli_engine::output::{extract_suggestions, read_language_from_db, truncate};
    use reqwest::StatusCode;
    use rusqlite::Connection;

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
    fn locales_cover_all_cli_messages() {
        let en = I18n {
            lang: "en".to_string(),
        };
        for code in ["ja", "vi", "zh"] {
            let loc = I18n {
                lang: code.to_string(),
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
                assert_ne!(loc.t(key), key, "missing {code} translation for {key}");
                assert_ne!(
                    loc.t(key),
                    en.t(key),
                    "{code} translation for {key} fell back to en"
                );
            }
        }
    }

    #[test]
    fn truncate_handles_vietnamese_cjk_and_emoji_without_utf8_panic() {
        let vi = "Hệ thống quản lý container đang hoạt động";
        let vi_trunc = truncate(vi, 12);
        assert_eq!(vi_trunc.chars().count(), 12);
        assert_eq!(vi_trunc, "Hệ thống qu…");

        let zh = "正在运行的容器列表测试字符串";
        let zh_trunc = truncate(zh, 6);
        assert_eq!(zh_trunc.chars().count(), 6);
        assert_eq!(zh_trunc, "正在运行的…");

        let ja = "実行中のコンテナ一覧を表示して";
        let ja_trunc = truncate(ja, 8);
        assert_eq!(ja_trunc.chars().count(), 8);
        assert_eq!(ja_trunc, "実行中のコンテ…");

        let emoji = "🐳🚀🔥✨📦🦀⚙️";
        let emoji_trunc = truncate(emoji, 4);
        assert_eq!(emoji_trunc.chars().count(), 4);
        assert_eq!(emoji_trunc, "🐳🚀🔥…");

        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exact", 5), "exact");
        assert_eq!(truncate("🐳🚀", 1), "…");
        assert_eq!(truncate("🐳🚀", 0), "");
    }

    #[test]
    fn cui_free_form_shorthand_routes_to_chat_with_default_a2a_session() {
        assert!(matches!(
            resolve_invocation("cui", &to_args(&[])).unwrap(),
            InvocationPlan::EmptyCui
        ));
        assert!(matches!(
            resolve_invocation("cui", &to_args(&["--help"])).unwrap(),
            InvocationPlan::DisplayMeta(_)
        ));
        assert!(matches!(
            resolve_invocation("cui", &to_args(&["-V"])).unwrap(),
            InvocationPlan::DisplayMeta(_)
        ));

        let plan = resolve_invocation(
            "cui",
            &to_args(&[
                "--yes",
                "--json",
                "--provider",
                "anthropic",
                "--model",
                "claude-sonnet-4-6",
                "restart",
                "nginx",
            ]),
        )
        .expect("parse free-form cui chat");

        match plan {
            InvocationPlan::Execute {
                use_json,
                command: Commands::Chat(chat),
            } => {
                assert!(use_json);
                assert!(chat.yes);
                assert_eq!(chat.session, DEFAULT_A2A_SESSION_ID);
                assert_eq!(chat.provider.as_deref(), Some("anthropic"));
                assert_eq!(chat.model.as_deref(), Some("claude-sonnet-4-6"));
                assert_eq!(chat.message, vec!["restart", "nginx"]);
            }
            other => panic!("expected Chat plan, got {other:?}"),
        }

        let eq_plan = resolve_invocation(
            "cui",
            &to_args(&[
                "-y",
                "--provider=openai",
                "--model=gpt-4o-mini",
                "--session=custom-thread",
                "list containers",
            ]),
        )
        .expect("parse inline = flags");

        match eq_plan {
            InvocationPlan::Execute {
                use_json,
                command: Commands::Chat(chat),
            } => {
                assert!(!use_json);
                assert!(chat.yes);
                assert_eq!(chat.session, "custom-thread");
                assert_eq!(chat.provider.as_deref(), Some("openai"));
                assert_eq!(chat.model.as_deref(), Some("gpt-4o-mini"));
                assert_eq!(chat.message, vec!["list containers"]);
            }
            other => panic!("expected Chat plan, got {other:?}"),
        }
    }

    #[test]
    fn cui_rejects_invalid_flags_and_missing_values() {
        assert!(resolve_invocation("cui", &to_args(&["--unknown", "hello"])).is_err());
        assert!(resolve_invocation("cui", &to_args(&["--provider"])).is_err());
        assert!(resolve_invocation("cui", &to_args(&["--model", "--yes", "hi"])).is_err());
        assert!(resolve_invocation("cui", &to_args(&["--yes"])).is_err());
        assert!(resolve_invocation("cui", &to_args(&["mcp", "--extra"])).is_err());

        let err = resolve_invocation("cui", &to_args(&["--json", "--unknown", "hi"]))
            .expect_err("expected error");
        assert!(err.json_requested);
    }

    #[test]
    fn global_json_flag_works_in_every_position() {
        for sample in [
            &["--json", "doctor"][..],
            &["doctor", "--json"][..],
            &["--json", "vm", "list"][..],
            &["vm", "--json", "list"][..],
            &["vm", "list", "--json"][..],
            &["container", "logs", "web-1", "--tail", "25", "--json"][..],
            &["--format", "json", "sessions", "list"][..],
        ] {
            let plan = resolve_invocation("cui", &to_args(sample))
                .unwrap_or_else(|e| panic!("failed parsing {sample:?}: {e:?}"));
            match plan {
                InvocationPlan::Execute { use_json, .. } => {
                    assert!(use_json, "expected use_json=true for {sample:?}");
                }
                other => panic!("unexpected plan for {sample:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn all_22_subcommands_plus_legacy_aliases_parse_cleanly() {
        let cases: &[&[&str]] = &[
            &["vm", "list"],
            &["instances", "status", "default"],
            &["config", "get", "--profile", "default"],
            &["presets", "list"],
            &["container", "list", "--all"],
            &["container", "stats", "api-1"],
            &["image", "list"],
            &["volume", "list"],
            &["network", "list"],
            &["compose", "autofix-history"],
            &["k8s", "pods", "-n", "kube-system"],
            &["k8s", "exec", "my-pod", "-n", "default", "--", "sh", "-c", "id"],
            &["kind", "list"],
            &["lima", "templates"],
            &["security", "rules"],
            &["models", "list"],
            &["alerts", "rules"],
            &["self-heal", "status"],
            &["memories", "search", "oom", "--limit", "3"],
            &["kb", "articles", "dns"],
            &["sessions", "list", "--include-a2a"],
            &["ui", "navigate", "containers", "--section", "running", "--toast", "Ready"],
            &["events", "--topics", "containers", "--limit", "5"],
            &["doctor"],
            &["chat", "--yes", "--mirror-ui", "Check Colima status"],
            &["mcp"],
            &["system", "check"],
            &["status"],
        ];

        for args in cases {
            let plan = resolve_invocation("cui", &to_args(args))
                .unwrap_or_else(|e| panic!("failed to parse {args:?}: {e:?}"));
            assert!(
                matches!(plan, InvocationPlan::Execute { .. }),
                "expected Execute plan for {args:?}"
            );
        }
    }

    #[test]
    fn vm_start_flags_default_to_none_to_preserve_colima_yaml() {
        let plan = resolve_invocation("colimaui", &to_args(&["vm", "start"]))
            .expect("parse colimaui vm start");
        match plan {
            InvocationPlan::Execute {
                command: Commands::Vm { action },
                ..
            } => {
                assert_eq!(
                    action,
                    VmCommand::Start {
                        profile: "default".to_string(),
                        cpus: None,
                        memory: None,
                        disk: None,
                        runtime: None,
                        arch: None,
                        vm_type: None,
                        k8s: false,
                    }
                );
            }
            other => panic!("unexpected plan: {other:?}"),
        }

        let stats = resolve_invocation("colimaui", &to_args(&["container", "stats"]))
            .expect("parse container stats");
        assert!(matches!(
            stats,
            InvocationPlan::Execute {
                command: Commands::Container {
                    action: ContainerCommand::Stats { id: None }
                },
                ..
            }
        ));
    }

    #[test]
    fn yaml_config_parser_handles_flat_and_nested_colima_keys() {
        let yaml = r#"
cpu: 6
memory: 12
vmType: vz
network:
  address: true
  dns:
    - 1.1.1.1
    - 8.8.8.8
kubernetes:
  enabled: true
"#;
        let changes = handlers_infra::parse_yaml_to_config_changes(yaml).expect("parse yaml");
        assert_eq!(changes.cpu, Some(6));
        assert_eq!(changes.memory, Some(12));
        assert_eq!(changes.vm_type.as_deref(), Some("vz"));
        assert_eq!(changes.network_address, Some(true));
        assert_eq!(
            changes.dns,
            Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );
        assert_eq!(changes.kubernetes, Some(true));
    }

    #[test]
    fn extract_suggestions_strips_tags_cleanly() {
        let raw = "Container nginx is healthy.\n[SUGGEST: Show nginx logs]\n[SUGGEST: Check CPU stats]";
        let (clean, sugs) = extract_suggestions(raw);
        assert_eq!(clean, "Container nginx is healthy.");
        assert_eq!(sugs, vec!["Show nginx logs", "Check CPU stats"]);
    }

    #[test]
    fn unwrap_response_json_unwraps_api_response_envelopes() {
        match unwrap_response_json(StatusCode::OK, r#"{"success":true,"data":{"ok":1}}"#) {
            HttpOutcome::Ok(v) => assert_eq!(v["ok"], 1),
            other => panic!("unexpected outcome: {other:?}"),
        }
        match unwrap_response_json(
            StatusCode::BAD_REQUEST,
            r#"{"success":false,"error":{"detail":"bad input"}}"#,
        ) {
            HttpOutcome::ApiErr(msg) => assert_eq!(msg, "bad input"),
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn read_language_from_db_reads_app_settings_table() {
        let tmp_path = std::env::temp_dir().join(format!(
            "cui-engine-lang-test-{}-{}.db",
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
        let _ = std::fs::remove_file(&tmp_path);
        assert_eq!(detected.as_deref(), Some("vi"));
    }
}
