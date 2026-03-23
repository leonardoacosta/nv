//! nv-daemon library facade.
//!
//! Exposes only the `tools` module for use by `nv-cli` standalone commands
//! (e.g. `nv check`). The full daemon runtime (channels, agent loop, HTTP
//! server) is not part of the public library API.

// These modules are used by the binary target (main.rs) and appear unused
// from the lib target's perspective. Allow dead_code for the facade.
#![allow(dead_code, unused_imports)]

// Declare all modules so the library target compiles cleanly.
// These are needed because `tools` imports from sibling modules.
mod account;
mod aggregation;
mod agent;
mod alert_rules;
mod bash;
mod callbacks;
mod channels;
mod claude;
mod conversation;
mod diary;
mod digest;
mod health;
mod http;
mod memory;
mod messages;
mod nexus;
mod obligation_detector;
mod obligation_store;
mod orchestrator;
mod query;
mod reminders;
mod scheduler;
mod speech_to_text;
mod shutdown;
#[allow(dead_code)]
mod state;
#[allow(dead_code)]
mod tailscale;
mod tts;
mod watchers;
mod worker;

/// Service tools — `Checkable` trait, `ServiceRegistry<T>`, `CheckResult`,
/// and the `check_all()` orchestrator.
pub mod tools;
