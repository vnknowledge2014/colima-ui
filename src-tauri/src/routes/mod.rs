pub mod announcements;
pub mod misc;
pub mod system;
pub mod instances;
/// colima.yaml editing plus the offline Help articles.
pub mod colima_config;
pub mod k8s;
pub mod containers;
pub mod images;
/// Background file/image transfers: container cp, image save, image load.
pub mod file_transfer;
/// Redacted diagnostic bundle behind "Report a problem".
pub mod diagnostics;
pub mod volumes;
pub mod networks;
/// Docker-layer topology graph. Unrelated to the Kubernetes resource graph,
/// which lives entirely in the frontend.
pub mod topology;
pub mod models;
/// History queries for the Activity page.
/// Self-healing rules, their log, and the switch that stops them.
pub mod self_heal;
/// The local record of what was done to this machine.
pub mod activity;
// `ws` held the HTTP terminal routes — never a WebSocket, despite the name.
// Terminal sessions are Tauri commands now; see commands/terminal.rs.
pub mod ai;
pub mod lima;
pub mod compose;
pub mod kb;
pub mod payloads;
pub mod capabilities;
/// Image vulnerability scanning and SBOM export.
pub mod security;
/// Sessions that run an untrusted image in a disposable isolated instance.
/// Host tool detection. Distinct from `capabilities`, which is the static
/// API schema published for AI agents — the two are unrelated contracts.
pub mod system_capabilities;
