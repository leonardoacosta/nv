# unified-messaging Specification

## Purpose
TBD - created by archiving change unify-conversation-streaming. Update Purpose after archive.
## Requirements
### Requirement: Persist all message exchanges
The system MUST persist all message exchanges to the messages table regardless of routing tier (keyword, embedding, or agent).

#### Scenario: Tier 1 keyword-routed message is persisted
Given a user sends a Telegram message that matches a keyword route
When the daemon dispatches to a fleet service and returns the response
Then both the user message and Nova's response are saved via saveExchange

#### Scenario: Tier 2 embedding-routed message is persisted
Given a user sends a Telegram message handled by embedding similarity
When the daemon dispatches to a fleet service and returns the response
Then both the user message and Nova's response are saved via saveExchange

### Requirement: Unified conversation context
The agent MUST see all recent messages from all channels when generating responses, not just messages from the current channel.

#### Scenario: Agent sees Telegram history when responding from dashboard
Given the user sent 3 messages via Telegram in the last hour
When the user sends a new message from the dashboard chat
Then the agent's conversation context includes the 3 Telegram messages with channel metadata

#### Scenario: Agent sees dashboard history when responding from Telegram
Given the user had a conversation on the dashboard chat
When the user sends a message via Telegram
Then the agent's conversation context includes the dashboard messages

### Requirement: WebSocket event system
The daemon MUST expose a /ws/events WebSocket endpoint that broadcasts message lifecycle events to connected clients.

#### Scenario: Dashboard receives streaming chunks via WebSocket
Given the dashboard is connected to /ws/events
When Nova streams a response to a Telegram message
Then the dashboard receives message.chunk events with text deltas in real time

#### Scenario: Dashboard receives new user message events
Given the dashboard is connected to /ws/events
When a user sends a message via Telegram
Then the dashboard receives a message.user event with sender, channel, and content

#### Scenario: WebSocket requires authentication
Given a client attempts to connect to /ws/events without a valid token
Then the connection is rejected with 401

### Requirement: Bidirectional message relay
Messages and responses MUST flow between Telegram and dashboard so both surfaces stay in sync.

#### Scenario: Dashboard response relayed to Telegram
Given the user sends a message from the dashboard chat
When the agent completes its response
Then the response is sent to Telegram via TelegramAdapter.sendMessage

#### Scenario: Telegram user message appears in dashboard
Given a user sends a message via Telegram
When the message is received by TelegramAdapter
Then a message.user WebSocket event is emitted and the dashboard chat displays it

#### Scenario: Dashboard user message noted in Telegram
Given a user sends a message from the dashboard
When the message is processed
Then a brief relay message appears in Telegram indicating "via Dashboard"

### Requirement: Dashboard chat unified stream
The dashboard chat page MUST display messages from all channels with channel identification and real-time streaming.

#### Scenario: Chat page shows cross-channel history
Given messages exist from both Telegram and dashboard channels
When the user loads the chat page
Then all messages appear in chronological order with channel badges

#### Scenario: Live Telegram stream visible in dashboard
Given the user is viewing the dashboard chat page
When Nova begins streaming a response to a Telegram message
Then the dashboard shows the streaming response in real-time with a Telegram channel badge

