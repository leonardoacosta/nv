# conversation-manager Specification

## Purpose
TBD - created by archiving change add-agent-sdk-integration. Update Purpose after archive.
## Requirements
### Requirement: ConversationManager.loadHistory

`ConversationManager` MUST expose `loadHistory(channelId: string, limit: number): Promise<Message[]>` which fetches the most recent messages for a channel and SHALL return them in ascending (chronological) order.

#### Scenario: Returns limited history

Given the `messages` table has 10 rows for `channelId: "telegram"`,
when `loadHistory("telegram", 3)` is called,
then the result is an array of exactly 3 `Message` objects in ascending `received_at` order.

#### Scenario: Empty channel

Given no rows exist for `channelId: "new-channel"`,
when `loadHistory("new-channel", 10)` is called,
then the result is an empty array `[]` and no error is thrown.

### Requirement: ConversationManager.saveExchange

`ConversationManager` MUST expose `saveExchange(channelId: string, userMsg: Message, assistantMsg: Message): Promise<void>` which SHALL persist both messages in a single Postgres transaction.

#### Scenario: Both messages persisted

Given a user `Message` with `id: "u1"` and an assistant `Message` with `id: "a1"`,
when `saveExchange("telegram", userMsg, assistantMsg)` is called,
then a subsequent `loadHistory("telegram", 10)` returns both messages.

#### Scenario: Assistant message normalized

Given an assistant `Message` is passed to `saveExchange`,
when the row is inserted,
then `senderId` and `senderName` are stored as `"nova"` regardless of what was passed in the `Message` object.

#### Scenario: Transaction rollback on partial failure

Given a simulated failure on the second INSERT (assistant message),
when `saveExchange` is called,
then neither row is persisted (transaction is rolled back atomically).

