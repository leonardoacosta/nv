# Reply Threading

## MODIFIED Requirements

### Requirement: Nova responses use reply_to

All Nova responses sent via `TelegramStreamWriter.finalize()` MUST set `reply_to_message_id` to the user's original Telegram messageId. For multi-chunk responses, only the first chunk SHALL use reply_to.

#### Scenario: Response to a plain message
Given user sends messageId 50 (no quote)
When Nova responds
Then the response message has reply_to_message_id = 50

#### Scenario: Response to a quoted message
Given user sends messageId 60 quoting messageId 40
When Nova responds
Then the response message has reply_to_message_id = 60 (replies to the user's message, not the quoted one)

#### Scenario: Multi-chunk response
Given Nova's response exceeds 4096 characters and splits into 3 chunks
When chunks are sent
Then only the first chunk has reply_to_message_id set; subsequent chunks are plain messages

### Requirement: Stream writer accepts replyToMessageId

`TelegramStreamWriter` constructor MUST accept an optional `replyToMessageId: number` parameter. When set, `finalize()` SHALL pass it to `adapter.sendMessage()` via the existing `replyToMessageId` option.

#### Scenario: Writer created with replyToMessageId
Given a TelegramStreamWriter constructed with replyToMessageId = 50
When finalize is called
Then adapter.sendMessage is called with options including replyToMessageId = 50

#### Scenario: Writer created without replyToMessageId (backward compatible)
Given a TelegramStreamWriter constructed without replyToMessageId
When finalize is called
Then adapter.sendMessage is called without replyToMessageId (current behavior)

### Requirement: Queue ack messages use reply_to

The "Queued" acknowledgment message MUST use `reply_to_message_id` pointing at the user's message, visually linking the ack to the message it acknowledges.

#### Scenario: Queue ack for a message
Given user sends messageId 70
When the queue sends an ack
Then the ack has reply_to_message_id = 70

## ADDED Requirements

### Requirement: Track Nova response messageId

After `finalize()` sends the first chunk, the returned Telegram messageId MUST be stored in the `messages` table alongside the thread context. This allows future user quotes of Nova's response to be resolved back to the thread.

#### Scenario: Nova responds and user quotes the response
Given Nova sends a response that gets Telegram messageId 51
And the response is saved to messages with thread_id = "telegram:123:50"
When user quotes messageId 51
Then ThreadResolver resolves it to thread "telegram:123:50"
