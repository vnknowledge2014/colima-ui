# E2E Test Infra: ColimaUI Daemon Connection & State Desync Resolution

## Test Philosophy
- Opaque-box, requirement-driven, independently verifiable.
- Methodology: Category-Partition + BVA + Pairwise + Workload Testing.

## Feature Inventory Coverage
- Socket Health & Liveness (Features 1–7): Mock sockets, EOF response, timeout response, 200 OK response, dead/live PID checks, 4-tier status calculation.
- Lifecycle & Async Subprocesses (Features 8–14): Partial Serde payloads, byte overflow normalization, process cancellation on timeout, CLI fallback label retention, compose start invocation.
- Watcher & Self-Healing (Features 15–21): Watcher 5s ping heartbeat, disconnect event emission on Tauri & SSE, cache clearing, `sweep_vm_unresponsive`, test DB isolation.
- Frontend Store & UI (Features 22–29): `normalizeContainer` label retention, Compose start button rendering, `$derived` container reactivity, cooldown clearance on error, Starting/Unhealthy status rendering.
- Verification & Release Gates (Feature 30): Cargo test, Clippy, npm check suite, git PR, application bundle build and install.

## Test Architecture
- Backend Unit & Integration Tests:
  - `RUSTC_WRAPPER="" cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`
- Backend Lint & Static Analysis:
  - `RUSTC_WRAPPER="" cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Frontend Unit & Static Checks:
  - `npm run lint`
  - `npm run typecheck`
  - `npm run check`
  - `npm run check:tokens`
  - `npm run check:i18n`
  - `npm test`
- Release & Installation Gate:
  - Verify app bundles at `/Applications/ColimaUI.app`, `~/Applications/ColimaUI.app`, and CLI at `/Users/mike/.cargo/bin/cui`.
