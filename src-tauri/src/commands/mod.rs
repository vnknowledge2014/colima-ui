pub mod announcements;
pub mod agent_loop;
pub mod ai_chat;
pub mod colima;
pub mod colima_config;
pub mod compose;
pub mod compose_diagnose;
pub mod compose_services;
pub mod containers;
pub mod diagnostics;
pub mod dockerfile_parse;
pub mod engine_resources;
pub mod file_transfer;
pub mod k8s_cluster;
pub mod k8s_resources;
pub mod kb_articles;
pub mod kind;
pub mod knowledge_bank;
pub mod kubernetes;
pub mod lima;
pub mod metrics_collector;

/// Rules that let the app repair a container without being asked each time.
pub mod self_heal;

/// What the user did to this machine, kept locally.
pub mod activity;
/// Test-only guard that every machine-changing command records what it did.
mod activity_coverage;
/// One timeline merged from the five stores that record what happened.
pub mod activity_feed;
pub mod models;
pub mod networks;
pub mod runtime;
pub mod searxng;
/// Image vulnerability scanning. Drives Trivy; never bundles it.
pub mod security_scan;
/// Configuration rules for images, and the score built from them.
pub mod security_catalog;
pub mod security_rules;
pub mod security_score;
pub mod shell_sandbox;
pub mod system;
pub mod system_capabilities;
pub mod terminal;
pub mod topology;
pub mod volumes;
