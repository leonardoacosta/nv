# Capability: Telegram Voice Upload

## ADDED Requirements

### Requirement: sendVoice Method
`TelegramClient` SHALL gain a `send_voice(chat_id, ogg_bytes, reply_to)` method that MUST upload OGG/Opus audio to Telegram's `sendVoice` endpoint using multipart/form-data. The voice message renders as an inline waveform bubble.

#### Scenario: Successful voice upload
Given valid OGG/Opus bytes and a chat_id
When `send_voice` is called
Then a multipart POST to `sendVoice` is made with the `voice` field containing the audio bytes (filename "voice.ogg", content-type "audio/ogg"), and the method returns the sent message ID.

#### Scenario: Voice upload failure
Given Telegram returns an error from sendVoice
When the upload is attempted
Then the error is logged at warn level. The text reply was already sent successfully.

#### Scenario: Reply threading
Given a reply_to message ID is provided
When `send_voice` is called
Then `reply_to_message_id` is included in the multipart form, threading the voice message under the original user message.

## MODIFIED Requirements

### Requirement: Dual Delivery Flow
After the existing text reply is sent (or the thinking message is edited with the response), the agent loop SHALL conditionally generate and send a voice follow-up.

#### Scenario: Voice enabled, short response
Given voice is enabled and response text is ≤ voice_max_chars
When the text reply has been sent successfully
Then TTS synthesis runs, and on success, `send_voice` delivers the audio as a reply to the same user message.

#### Scenario: Voice enabled, long response
Given voice is enabled but response text exceeds voice_max_chars
When the text reply has been sent
Then voice synthesis is skipped entirely. No TTS API call is made.

#### Scenario: Voice disabled
Given voice is disabled (config or runtime toggle)
When any response is generated
Then no TTS or voice operations occur. Behavior is identical to current.
