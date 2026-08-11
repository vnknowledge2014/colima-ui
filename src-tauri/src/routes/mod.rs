pub mod misc;
pub mod system;
pub mod instances;
/// colima.yaml editing plus the offline Help articles.
pub mod colima_config;
pub mod k8s;
pub mod containers;
pub mod images;
pub mod volumes;
pub mod networks;
pub mod models;
// `ws` held the HTTP terminal routes — never a WebSocket, despite the name.
// Terminal sessions are Tauri commands now; see commands/terminal.rs.
pub mod ai;
pub mod lima;
pub mod compose;
pub mod kb;
pub mod payloads;
pub mod capabilities;
/// Host tool detection. Distinct from `capabilities`, which is the static
/// API schema published for AI agents — the two are unrelated contracts.
pub mod system_capabilities;
