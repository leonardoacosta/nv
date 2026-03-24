# Nexus — Audit Memory

## Key Modules
- nexus/client.rs (26.8K) — NexusClient, multi-agent connect, session queries
- nexus/connection.rs — gRPC channel lifecycle, ConnectionStatus
- nexus/stream.rs — event streaming
- nexus/tools.rs — tool definitions for session introspection
- nexus/progress.rs — progress tracking
- nexus/notify.rs — session notifications
- nexus/watchdog.rs — heartbeat, health monitoring
- query/gather.rs — session metadata collection
- query/synthesize.rs — multi-session aggregation
- query/format.rs — display formatting
- query/followup.rs — follow-up suggestions

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
