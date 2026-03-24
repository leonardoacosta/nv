# Capability: Session Stability

## MODIFIED Requirements

### Requirement: Claude CLI error handling
The worker MUST retry once on malformed JSON from Claude CLI, then fall back to cold-start. Session timeout MUST be configurable via nv.toml.

#### Scenario: Malformed JSON recovery
**Given** Claude CLI returns invalid JSON
**When** the worker processes the response
**Then** it retries once, and if still invalid, starts a cold session

#### Scenario: Channel reconnection
**Given** Telegram WebSocket disconnects
**When** the disconnect is detected
**Then** reconnection attempts with exponential backoff (1s, 2s, 4s, 8s, max 60s)

#### Scenario: Memory read before response
**Given** Nova receives a message
**When** building the system prompt for Claude
**Then** the prompt includes "Read your memory files before answering"
