# Ping Intercept

## ADDED Requirements

### Requirement: Bare ping detection and pong response

The daemon MUST detect messages matching `/^ping$/i` before the routing cascade and immediately reply with "pong" via Telegram. The intercept SHALL skip queue entry, agent SDK calls, conversation save, obligation detection, and diary writes.

#### Scenario: User sends "ping" on Telegram
Given a Telegram message with text "ping"
When the message reaches the daemon routing logic
Then the daemon replies "pong" to the same chat
And no job is enqueued
And no agent SDK call is made
And no conversation is saved to the messages table

#### Scenario: User sends "Ping" (case insensitive)
Given a Telegram message with text "Ping" or "PING"
When the message reaches the daemon routing logic
Then the daemon replies "pong" (same behavior as lowercase)

#### Scenario: User sends "ping hello" (not bare ping)
Given a Telegram message with text "ping hello"
When the message reaches the daemon routing logic
Then the message proceeds through normal routing (ping intercept does not match)

#### Scenario: Ping response uses reply_to
Given a Telegram message with messageId 100 and text "ping"
When the daemon replies with "pong"
Then the reply MUST have reply_to_message_id = 100
