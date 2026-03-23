# Proposal: Harden Session Stability

## Change ID
`harden-session-stability`

## Summary
Fix Claude CLI session management (retry, timeout, error reporting), channel reconnection (exponential backoff), and memory consistency (system prompt reads memory before responding).

## Context
- Extends: worker.rs, agent.rs, claude.rs, channels/telegram/mod.rs, channels/discord/gateway.rs, system-prompt.md

## Motivation
Session crashes, tool failures silently swallowed, and memory loss between sessions are the top pain points. These are reliability bugs, not feature gaps.

## Requirements

### Req-1: Claude CLI resilience
Retry on malformed JSON once then cold-start fallback. Configurable session timeout. Graceful error reporting to user channel.

### Req-2: Channel reconnection
Exponential backoff on Telegram/Discord/Teams disconnect with automatic reconnection.

### Req-3: Memory consistency
System prompt explicitly instructs Claude to read memory files before every response.

## Scope
- **IN**: CLI error handling, channel reconnection, system prompt memory instruction
- **OUT**: New features, new tools, Nexus changes

## Impact
| Area | Change |
|------|--------|
| worker.rs | Retry logic, timeout handling |
| agent.rs | Retry logic, memory instruction |
| claude.rs | JSON parse retry, cold-start fallback |
| channels/*/mod.rs | Reconnection with backoff |
| system-prompt.md | Memory read instruction |

## Risks
| Risk | Mitigation |
|------|-----------|
| Retry loops on persistent failures | Max 1 retry then cold-start, not infinite |
