# Spec: Conversation Memory

## ADDED Requirements

### Requirement: ConversationStore

The system MUST provide a `ConversationStore` struct that holds a bounded deque of Claude API
message pairs (user message + assistant response including tool_use/tool_result blocks).
The store MUST be thread-safe via `Arc<Mutex<...>>` and stored in `SharedDeps`.

#### Scenario: Store accumulates turns across workers

**Given** Worker A processes a user message and receives Claude's response
**When** Worker A pushes the turn pair to ConversationStore
**And** Worker B processes the next user message
**Then** Worker B's conversation_history includes Worker A's turn pair before the new message
**And** Claude sees the prior exchange and does not repeat itself

#### Scenario: Session expires after inactivity

**Given** the last turn was pushed 11 minutes ago (SESSION_TIMEOUT = 600s)
**When** a new worker loads turns from the store
**Then** the store is cleared first (session expired)
**And** the worker starts with a fresh conversation history

#### Scenario: History bounded by turns and chars

**Given** the store has 25 turns totaling 60,000 chars
**When** a new turn is pushed
**Then** the oldest turns are evicted until turns ≤ 20 and total chars ≤ 50,000

### Requirement: Tool result truncation

The system MUST truncate individual tool_result content beyond 1,000 chars when storing
assistant responses in the ConversationStore, to prevent history bloat from large tool outputs.

#### Scenario: Large tool result is truncated in stored history

**Given** a tool_result block contains 5,000 chars of Jira search results
**When** the turn is pushed to ConversationStore
**Then** the stored tool_result content is truncated to 1,000 chars with "... [truncated]"
**And** the original response sent to the user is unaffected

## MODIFIED Requirements

### Requirement: Enhanced MessageStore context

The system MUST raise per-message truncation from 500 to 2,000 chars in
`format_recent_for_context()` and MUST add turn-pair markers so the injected text shows
clear user→assistant grouping.

#### Scenario: Nova response preserved in recent context

**Given** Nova sent a 1,500-char response in the previous turn
**When** the next worker loads `<recent_messages>` context
**Then** the response is included in full (not truncated at 500 chars)

#### Scenario: Turn structure is visible

**Given** 3 recent exchanges exist in MessageStore
**When** formatted for context injection
**Then** output groups messages as `[HH:MM] Leo: ...` / `[HH:MM] Nova: ...` pairs
with blank line separators between turns
