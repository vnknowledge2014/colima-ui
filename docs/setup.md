# Developer Setup & Build Runbook

Canonical checklist for setting up a ColimaUI development environment, running CI-equivalent quality gates, packaging desktop bundles, and compiling the standalone CLI binaries.

---

## 1. Prerequisites

### Toolchain
- **Node.js `>= 22.22.2`** with **`npm`** (`.nvmrc` pins the release line; `jsdom` and `undici` in the test stack require Node 22.22.2+).
- **Rust stable toolchain** via [`rustup`](https://rustup.rs/) (`cargo`, `rustc`, `clippy`).
- **Platform native dependencies**:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Ubuntu / Debian Linux**:
    ```bash
    sudo apt-get update
    sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
    ```

### Runtime Tools
- **Required for container workflows**: `colima`, `docker`, `limactl`
- **Optional (feature-scoped)**:
  - `kubectl` and `kind` — Kubernetes resource browser, pod exec, port forwarding, and Kind cluster management
  - `trivy` — Local container vulnerability scanning and SBOM generation on the Security page
  - `ollama` — Local LLM inference and the Models management page

---

## 2. Install & Git Hooks

Clone the repository and install Node dependencies with `npm`:

```bash
git clone https://github.com/vnknowledge2014/colima-ui.git
cd colima-ui
npm install
```

`npm install` automatically triggers the `prepare` lifecycle script (`bash scripts/setup-hooks.sh`), which sets `git config core.hooksPath .githooks` and activates the pre-push lint gate.

---

## 3. Development Mode

Launch the full desktop application (Rust backend + native Tauri webview + Vite HMR):

```bash
npm run tauri dev
```

What starts in development mode:
1. **Native Desktop Window** — Uses Tauri IPC (`invoke` + Tauri events) directly.
2. **Local HTTP & SSE API Server** — Binds `127.0.0.1:11420` (falling back across `11420–11429` if occupied).
3. **Browser UI Development Surface** — Debug builds print a ready-to-open URL to stdout:
   ```text
   Browser mode: http://localhost:1420/#token=colima-...
   ```
   Opening that URL boots the Vite frontend against the live Rust backend; the frontend reads `#token=...` once and strips the fragment from the address bar. (Release builds do **not** serve static frontend assets on `:11420` and never print the token to stdout.)
4. **Local CLI Session Credentials** — On startup, the app writes `~/.colima-ui/api_token` and `~/.colima-ui/api_port` with `0600` permissions inside `~/.colima-ui` (`0700`) so `cui` and authorized same-user local agents can authenticate with `Authorization: Bearer <token>`, then removes them cleanly on exit.

To work on frontend layout/styles alone without compiling Rust:

```bash
npm run dev
```

---

## 4. Quality Gates (CI Parity)

Run every gate below before opening a pull request. `.githooks/pre-push` and `.github/workflows/ci.yml` enforce the exact same checks:

```bash
npm run lint          # ESLint (--max-warnings=0): dead code, unused imports/vars, unsafe patterns
npm run typecheck     # TypeScript compiler check (tsc --noEmit)
npm run check         # Svelte 5 component & rune type check (svelte-check)
npm run check:tokens  # CSS custom property audit (no undefined var() references)
npm run check:i18n    # Locale parity across en, ja, vi, and zh
npm test              # Vitest unit & component test suite
npm run lint:rust     # cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

> **Integration tests:** Rust tests that require a running Docker daemon or `trivy` binary are marked `#[ignore]` so `cargo test --lib` stays deterministic offline. Run them explicitly when those tools are active:
> ```bash
> cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
> ```

---

## 5. Production Packaging & CLI Binaries

### Local Unsigned macOS `.app` Bundle

`tauri.conf.json` enables updater artifacts by default, which requires `TAURI_SIGNING_PRIVATE_KEY`. To package a local `.app` bundle without a signing key, disable updater artifact generation inline:

```bash
npx tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

Output bundle: `src-tauri/target/release/bundle/macos/ColimaUI.app`.

> **macOS Gatekeeper note:** Locally compiled `.app` bundles run directly on the build machine. When installing an unsigned `.dmg` downloaded from a browser onto macOS, clear the quarantine attribute after copying to `/Applications`:
> ```bash
> xattr -cr /Applications/ColimaUI.app
> ```

### Signed Release Builds

When building distributable release artifacts with Tauri updater signatures (and optional Apple notarization), export the signing credentials before running `npm run tauri build`:

| Variable | Purpose |
|----------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents or path of the Tauri updater private key (required when `createUpdaterArtifacts` is `true`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for `TAURI_SIGNING_PRIVATE_KEY` (if encrypted) |
| `APPLE_SIGNING_IDENTITY` / `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` | Optional Apple Developer ID code-signing identity & P12 certificate |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Optional Apple notarization credentials |

```bash
npm run tauri build
```

### Standalone CLI Binaries (`cui` `v0.3.0` & `colimaui` Alias), Native A2A & 22-Tool MCP Server

GitHub Releases publish prebuilt standalone CLI archives alongside desktop installers:
- `cui-{aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-gnu}.tar.gz` (primary unified CLI-First, A2A & MCP binary)
- `colimaui-{aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-gnu}.tar.gz` (backward-compatible CLI & MCP alias)

You can install or compile both Rust CLI targets locally from `src-tauri/src/bin/`:

1. **`cui` (`v0.3.0`)** — Unified CLI-First control plane (**22 subcommands**: 20 deterministic `< 5ms` direct subcommands + `chat` + `mcp`), A2A session client, and 22-tool MCP stdio server. Proxies over `127.0.0.1:11420..11429` (`~/.colima-ui/api_port` + `0600` `~/.colima-ui/api_token`) when ColimaUI Desktop is open, and automatically falls back to standalone native Rust execution when Desktop is closed. Passing `--json` in any position emits `{"ok": true, "data": ...}` or `{"ok": false, "error": "..."}`:
   ```bash
   # Install into ~/.cargo/bin/cui (or build in src-tauri/target/release/cui)
   cargo install --path ~/Documents/colima-ui/src-tauri --bin cui
   cargo build --manifest-path src-tauri/Cargo.toml --bin cui --release

   # Verify installation & environment health
   cui --version          # Expected: cui 0.3.0
   cui doctor --json

   # Instant < 5ms deterministic subcommands (20 direct domains):
   # vm | config | presets | container | image | volume | network | compose | k8s | kind |
   # lima | security | models | alerts | self-heal | memories (kb) | sessions | ui | events | doctor
   cui vm list --json
   cui container list --all --json
   cui k8s pods -n default --json
   cui ui navigate kubernetes --section pods --toast "Inspecting pods" --json

   # Isolated A2A multi-turn chat (defaults to `colima-a2a-default-session`; --mirror-ui mirrors into AiChatPanel)
   cui chat "Check Colima status and list exited containers" --session colima-a2a-default-session --json
   cui chat "Restart the exited redis container" --session colima-a2a-default-session --yes --mirror-ui --json
   cui "Summarize K8s pod health" --provider anthropic --model claude-sonnet-4-6 --json
   cui mcp
   ```

2. **`colimaui`** — Backward-compatible direct subcommand CLI & 22-tool MCP stdio server alias (gated behind the `cli` Cargo feature for `clap`):
   ```bash
   cargo build --manifest-path src-tauri/Cargo.toml --bin colimaui --features cli --release
   # Binary: src-tauri/target/release/colimaui
   colimaui mcp
   ```

#### Connecting External AI Hosts via MCP (`cui mcp` / `colimaui mcp`) & Native A2A

Both `cui mcp` and `colimaui mcp` expose **22 typed JSON-RPC 2.0 MCP tools** over `stdin`/`stdout` (`colima_list_instances`, `colima_instance_action`, `docker_list_containers`, `docker_container_action`, `docker_container_logs`, `docker_list_images`, `docker_pull_image`, `docker_list_volumes`, `docker_list_networks`, `compose_list_projects`, `k8s_list_pods`, `k8s_cluster_status`, `environment_resolutions`, `system_diagnostics`, `colima_ai_ask`, `security_scan_image`, `compose_diagnose_autofix`, `lima_manage`, `colima_config`, `kb_memories`, `chat_sessions`, `ui_navigate`), working seamlessly whether ColimaUI Desktop is open or closed:

- **Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`), **Cursor** (`~/.cursor/mcp.json`), **Antigravity** (`~/.gemini/antigravity/mcp_config.json`), **Windsurf** (`~/.codeium/windsurf/mcp_config.json`):
  ```json
  {"mcpServers": {"colima-ui": {"command": "cui", "args": ["mcp"]}}}
  ```
- **Zed** (`~/.config/zed/settings.json`):
  ```json
  {"context_servers": {"colima-ui": {"command": {"path": "cui", "args": ["mcp"]}}}}
  ```
- **Claude Code**:
  ```bash
  claude mcp add colima-ui -- cui mcp
  ```
- **Native HTTP A2A Discovery & JSON-RPC 2.0** (authenticated via `~/.colima-ui/api_token`; `GET /api/health` remains the sole public route):
  ```bash
  PORT="$(cat ~/.colima-ui/api_port 2>/dev/null || echo 11420)"
  TOKEN="$(cat ~/.colima-ui/api_token)"
  curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/.well-known/agent.json"
  ```

---

## 6. Automated GitHub Releases (`release.yml`)

Every push or merged PR on `main` that passes `.github/workflows/ci.yml` automatically triggers `.github/workflows/release.yml` (`on: workflow_run`):

1. **`prepare` job** — Verifies the commit is on `main` and passed `ci`, computes the next SemVer version (`max(package.json, highest stable git tag)` with automatic patch increment when that tag already exists), pushes the `vX.Y.Z` tag, generates PR release notes since the previous published release, and creates **one single Draft GitHub Release** (`release_id`).
2. **`build` matrix job** — Compiles `macOS ARM64 (.dmg)`, `macOS x86_64 (.dmg)`, and `Linux x86_64 (.deb, .AppImage)` plus standalone CLI archives (`cui-{aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-gnu}.tar.gz` and `colimaui-{aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-gnu}.tar.gz`) against the tag and uploads all artifacts to `release_id`:
   - If `TAURI_SIGNING_PRIVATE_KEY` is configured in GitHub Actions Secrets, signed updater bundles and `latest.json` are generated and uploaded automatically.
   - If `TAURI_SIGNING_PRIVATE_KEY` is absent, `createUpdaterArtifacts` is automatically disabled for the CI build so unsigned installers publish cleanly without failing.
3. **`publish` job** — Verifies all platform desktop bundles and standalone CLI tarballs are attached to `release_id` before flipping the Draft release to **Published (`Latest`)**.

To prepare a `minor` or `major` version jump on a feature branch before opening a PR (or to cut an explicit `-rc.N` / `-beta.N` tag), use `./scripts/release.sh`.
