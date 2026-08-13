pub mod announcements;
pub mod agent_loop;
pub mod ai_chat;
pub mod colima;
pub mod colima_config;
pub mod compose;
pub mod compose_autofix;
pub mod compose_autofix_apply;
pub mod compose_autofix_fixers;
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
/// Durable metrics history: the Pro half of the Activity page.
pub mod metrics_store;
/// Threshold alerts over the metrics stream.
pub mod alerts;

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
/// Turns a failing security rule into a concrete Dockerfile edit.
pub mod security_autofix;
/// Configuration rules for images, and the score built from them.
pub mod security_catalog;
pub mod security_rules;
/// Scan scores kept over time, in a store with its own lifetime.
pub mod security_history;
/// The minimum posture a user is willing to accept from an image.
pub mod security_policy;
pub mod security_score;
/// Scheduled rescans of running images. Off unless the user asks.
pub mod security_watch;
/// Orders the failing rules by what fixing each one is worth.
pub mod security_triage;
/// Reads Falco's output. Detects nothing itself — Falco does the detecting.
pub mod falco_bridge;
/// Builds the prompt that asks a model to explain events. Never a verdict.
pub mod falco_triage;
/// Runs an untrusted image in a throwaway instance and records what it did.
pub mod detonation;
pub mod shell_sandbox;
pub mod system;
pub mod system_capabilities;
pub mod terminal;
pub mod topology;
pub mod volumes;
