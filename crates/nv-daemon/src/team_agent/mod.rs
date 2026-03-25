//! Team-agent subprocess management.
//!
//! When `use_team_agents = true` in config, the daemon uses
//! `TeamAgentDispatcher` instead of Nexus gRPC to spawn CC sessions.

pub mod dispatcher;
pub mod session;

pub use dispatcher::TeamAgentDispatcher;
