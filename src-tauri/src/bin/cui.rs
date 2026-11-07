//! Thin entrypoint for the `cui` CLI-First DevOps, Headless AI & MCP binary (`v0.3.0`).

fn main() {
    colima_ui_lib::cli_engine::init_process_env();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize Tokio runtime");
    let exit_code = rt.block_on(colima_ui_lib::cli_engine::run_from_env("cui"));
    std::process::exit(exit_code);
}
