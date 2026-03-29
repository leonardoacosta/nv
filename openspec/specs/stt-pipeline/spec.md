# stt-pipeline Specification

## Purpose
TBD - created by archiving change add-voice-transcription. Update Purpose after archive.
## Requirements
### Requirement: SttError Class
The daemon SHALL define a typed `SttError` class in `packages/daemon/src/features/stt/client.ts`
that extends `Error` and carries a `code` discriminant (`"missing_key" | "download_failed" |
"api_error" | "empty_transcript"`). All STT failures MUST be surfaced as `SttError` instances so
callers can branch on `code` without string-matching error messages.

#### Scenario: Missing API key at call time
Given `process.env["ELEVENLABS_API_KEY"]` is undefined
When `transcribe(fileUrl)` is called
Then it throws `SttError` with `code: "missing_key"` and message `"ELEVENLABS_API_KEY not set"`

#### Scenario: Key present but empty string
Given `process.env["ELEVENLABS_API_KEY"]` is `""`
When `transcribe(fileUrl)` is called
Then it throws `SttError` with `code: "missing_key"`

---

### Requirement: Audio Download
The `transcribe()` function MUST fetch the voice audio from the provided `fileUrl` using a 30-second
HTTP timeout. The raw response bytes SHALL be passed directly to the ElevenLabs STT call without
writing to disk.

#### Scenario: Successful download
Given a valid Telegram CDN file URL and reachable network
When `transcribe(fileUrl)` fetches the audio
Then it receives raw `Buffer` bytes and proceeds to the ElevenLabs POST

#### Scenario: Download network error (DNS / connection refused)
Given the download URL is unreachable
When `transcribe(fileUrl)` attempts to fetch
Then it throws `SttError` with `code: "download_failed"` containing the underlying network error message

#### Scenario: Download timeout (>30s)
Given the CDN server does not respond within 30 seconds
When `transcribe(fileUrl)` is waiting for bytes
Then the request is aborted and `SttError` with `code: "download_failed"` and message `"download timeout"` is thrown

#### Scenario: Download returns non-200 status
Given the CDN URL returns HTTP 403 or 404
When `transcribe(fileUrl)` reads the response status
Then it throws `SttError` with `code: "download_failed"` including the HTTP status in the message

---

### Requirement: ElevenLabs STT API Call
The `transcribe()` function MUST POST the audio bytes to
`https://api.elevenlabs.io/v1/speech-to-text/convert` with:
- Header `xi-api-key: <ELEVENLABS_API_KEY>`
- Header `Content-Type: audio/ogg`
- Body: raw audio bytes
- HTTP timeout of 30 seconds

The function SHALL parse the `text` field from the JSON response body and return it as a `string`.

#### Scenario: Successful transcription
Given valid OGG audio bytes and a live ElevenLabs API key
When `transcribe(fileUrl)` posts to the STT endpoint
Then it returns the `text` string from the response JSON (e.g. `"Check the status of tribal cities"`)

#### Scenario: ElevenLabs API returns non-2xx
Given the ElevenLabs API responds with HTTP 429 (rate limit) or 500
When `transcribe(fileUrl)` reads the response status
Then it throws `SttError` with `code: "api_error"` including the HTTP status and response body excerpt in the message

#### Scenario: ElevenLabs API returns 2xx with empty transcript
Given the API responds with `{ "text": "" }` (silence detected)
When `transcribe(fileUrl)` reads the `text` field
Then it throws `SttError` with `code: "empty_transcript"` and message `"ElevenLabs returned empty transcript"`

#### Scenario: ElevenLabs API call times out (>30s)
Given the ElevenLabs API does not respond within 30 seconds
When `transcribe(fileUrl)` is awaiting the POST response
Then the request is aborted and `SttError` with `code: "api_error"` and message `"STT API timeout"` is thrown

---

### Requirement: normalizeVoiceMessage Wiring
`normalizeVoiceMessage()` in `packages/daemon/src/channels/telegram.ts` MUST call `transcribe(fileUrl)`
after URL resolution and write the returned transcript into both `text` and `content` fields of the
returned `Message`. The `fileUrl` SHALL remain in `metadata` regardless of transcription outcome.
The function signature (`(msg, bot) => Promise<Message>`) MUST NOT change.

#### Scenario: Successful transcription flow
Given a Telegram voice message with a valid `file_id`
When `normalizeVoiceMessage()` is called
Then `bot.getFileLink()` resolves a `fileUrl`, `transcribe(fileUrl)` returns a transcript string,
and the returned `Message` has `text` and `content` equal to that transcript, with `metadata.fileUrl` set

#### Scenario: fileUrl resolution failed (existing catch block)
Given `bot.getFileLink()` throws (network error or API error)
When `normalizeVoiceMessage()` catches the error
Then `transcribe()` is NOT called, a `warn`-level log is emitted, and the returned `Message` has
`text` and `content` set to `"[Voice message — could not retrieve audio file]"`

#### Scenario: Transcription fails with SttError
Given `fileUrl` is resolved but `transcribe()` throws `SttError`
When `normalizeVoiceMessage()` catches the `SttError`
Then the error is logged at `warn` level (including `SttError.code`), and the returned `Message`
has `text` and `content` set to `"[Voice message — transcription unavailable]"`;
no exception propagates to the caller

#### Scenario: Transcription fails with unexpected error
Given `transcribe()` throws a non-SttError (e.g. unexpected JSON parse failure)
When `normalizeVoiceMessage()` catches the error
Then the error is logged at `warn` level, and the returned `Message` has `text` and `content` set
to `"[Voice message — transcription unavailable]"`; the handler still calls `onMessageCallback`

---

### Requirement: Routing Cascade Receives Non-Empty Text
After this change, the `MessageRouter.route(text)` MUST receive a non-empty string for all voice
message paths (transcript or fallback). The empty-string path that caused silent routing failure
SHALL be eliminated.

#### Scenario: Transcript flows through routing cascade
Given a successfully transcribed voice message with `text: "remind me to call Leo at 5pm"`
When `MessageRouter.route(text)` is called
Then tier-1 keyword matching or tier-3 Agent SDK processes the transcript identically to a typed message

#### Scenario: Fallback text flows through routing cascade
Given transcription was unavailable and `text` is `"[Voice message — transcription unavailable]"`
When `MessageRouter.route(text)` is called
Then tier-3 Agent SDK receives the fallback text and MAY reply asking the user to retype their message

