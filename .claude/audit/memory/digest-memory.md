# Digest — Audit Memory

## Route Inventory
- POST /digest (trigger immediate digest)

## Key Modules
- digest/gather.rs — collect alerts, obligations, sessions
- digest/synthesize.rs — Claude summarization
- digest/format.rs — HTML + plain text templating
- digest/actions.rs — suggested actions
- digest/state.rs — DigestStateManager (hash suppression)
- scheduler.rs — cron triggers, morning briefing @ 7am

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
