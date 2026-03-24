# Watchers — Audit Memory

## Key Modules
- watchers/deploy_watcher.rs — Vercel deploy failures
- watchers/sentry_watcher.rs — Sentry spike detection
- watchers/stale_ticket_watcher.rs — stale beads issues
- watchers/ha_watcher.rs — Home Assistant anomalies
- watchers/mod.rs — spawn_watchers(), run_watcher_cycle()
- alert_rules.rs — rule storage, enabled flag, cooldown
- obligation_store.rs — SQLite CRUD, status machine
- obligation_detector.rs — commitment detection from messages

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
