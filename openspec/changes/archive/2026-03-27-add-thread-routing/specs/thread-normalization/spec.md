# Thread Normalization

## MODIFIED Requirements

### Requirement: Capture reply_to_message metadata

The Telegram message normalization functions (`normalizeTextMessage`, `normalizeVoiceMessage`, `normalizePhotoMessage`) MUST extract `reply_to_message.message_id` from the raw Telegram message and populate it on the normalized `Message` object.

#### Scenario: User sends a plain message (no quote)
Given a Telegram message with no `reply_to_message` field
When the message is normalized
Then `replyToMessageId` is `undefined`
And `threadId` is set to the message's own `messageId` (new thread root)

#### Scenario: User quotes a previous message
Given a Telegram message with `reply_to_message.message_id = 42`
When the message is normalized
Then `replyToMessageId` is `42`
And `threadId` is resolved by walking the reply chain to the root

#### Scenario: User quotes a message that itself was a reply
Given message A (root, messageId=10), message B (reply to A, messageId=20), message C (reply to B, messageId=30)
When message C is normalized
Then `replyToMessageId` is `20`
And `threadId` is `10` (the root of the chain)

### Requirement: Thread root resolution

A `ThreadResolver` component MUST maintain an in-memory cache mapping Telegram `messageId` to `threadRootMessageId`. On cache miss, it SHALL query the `messages` table to walk the `reply_to_message_id` chain to the root.

#### Scenario: Cache hit
Given messageId 42 is cached with threadRoot 10
When resolveThread(42) is called
Then it returns 10 without a DB query

#### Scenario: Cache miss with DB fallback
Given messageId 42 is not cached but exists in messages with reply_to_message_id = 20, and messageId 20 has reply_to_message_id = NULL (root)
When resolveThread(42) is called
Then it queries the DB, walks the chain 42 to 20, returns 20, and caches both entries

### Requirement: Message type extension

The `Message` interface in `types.ts` MUST add two fields:
- `threadId: string | undefined` — the resolved thread root identifier (`chatId:messageId`)
- `replyToMessageId: number | undefined` — the Telegram messageId being replied to

#### Scenario: Message with thread fields populated
Given a normalized message from a quoted Telegram message
When the Message object is inspected
Then `threadId` is a string in format `chatId:rootMessageId`
And `replyToMessageId` is the quoted message's Telegram messageId

## ADDED Requirements

### Requirement: Thread-scoped conversation history

`ConversationManager.loadHistory` MUST accept an optional `threadId` parameter. When provided, it SHALL filter to messages with matching `thread_id` column. When omitted, it falls back to the current channel-scoped behavior (backward compatible).

#### Scenario: Load history for a specific thread
Given 5 messages in thread "telegram:123:10" and 3 messages in thread "telegram:123:20"
When loadHistory("telegram:123", 20, "telegram:123:10") is called
Then it returns only the 5 messages from thread "telegram:123:10"

#### Scenario: Load history without thread (legacy)
Given 8 total messages in channel "telegram:123" across multiple threads
When loadHistory("telegram:123", 20) is called
Then it returns all 8 messages (backward compatible)
