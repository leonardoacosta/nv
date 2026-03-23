# Capability: Proactive Watchers

## ADDED Requirements

### Requirement: proactive watchers
The system MUST implement: Cron-triggered watchers: deploy_watcher, sentry_watcher, stale_ticket_watcher, ha_watcher. Each evaluates alert rules and creates obligations.

#### Scenario: Core functionality
**Given** the feature is implemented per PRD FR-7
**When** the system operates normally
**Then** the feature works as specified in the PRD
