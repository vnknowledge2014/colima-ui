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

### Standalone CLI Binaries

ColimaUI ships two Rust CLI targets in `src-tauri/src/bin/`:

1. **`cui`** — Natural-language headless AI client that reads `~/.colima-ui/api_token`, discovers the active backend port (`11420–11429`), and calls `POST /api/cli/chat`:
   ```bash
   cargo build --manifest-path src-tauri/Cargo.toml --bin cui --release
   # Binary: src-tauri/target/release/cui
   ```

2. **`colimaui`** — Direct subcommand CLI (gated behind the `cli` Cargo feature for `clap`):
   ```bash
   cargo build --manifest-path src-tauri/Cargo.toml --bin colimaui --features cli --release
   # Binary: src-tauri/target/release/colimaui
   ```
