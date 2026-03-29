# Implementation Tasks

<!-- beads:epic:pending -->

## DB Batch

(no database changes required)

## API Batch

- [x] [2.1] [P-1] Create `packages/daemon/src/features/stt/client.ts` -- define `SttError` class extending `Error` with `code: "missing_key" | "download_failed" | "api_error" | "empty_transcript"` discriminant; export `transcribe(fileUrl: string): Promise<string>` that reads `ELEVENLABS_API_KEY` from env (throw `SttError code: "missing_key"` if absent or empty), downloads audio bytes with 30s timeout (throw `SttError code: "download_failed"` on error/non-200/timeout), POSTs raw bytes to `https://api.elevenlabs.io/v1/speech-to-text/convert` with `xi-api-key` header and `Content-Type: audio/ogg` and 30s timeout (throw `SttError code: "api_error"` on non-2xx/timeout), parses `text` field from response JSON (throw `SttError code: "empty_transcript"` if blank) [owner:api-engineer] [beads:pending]
- [x] [2.2] [P-1] Wire `transcribe()` into `normalizeVoiceMessage()` in `packages/daemon/src/channels/telegram.ts` -- after `fileUrl` resolution, call `transcribe(fileUrl)` in a try/catch; on success set `text` and `content` to the transcript; on `SttError` or any error log at `warn` level (include `SttError.code` if applicable) and set `text`/`content` to `"[Voice message — transcription unavailable]"`; if `fileUrl` was never resolved set `text`/`content` to `"[Voice message — could not retrieve audio file]"` and skip the `transcribe()` call entirely; preserve `metadata.fileUrl` in all paths [owner:api-engineer] [beads:pending]

## UI Batch

(no UI changes required)

## E2E Batch

- [x] [4.1] [P-1] Unit tests for `packages/daemon/src/features/stt/client.ts` -- test `SttError` construction for each `code`; mock `fetch` to simulate: successful 200 response with `{ text: "hello" }`, download non-200 (403), download timeout, ElevenLabs non-2xx (429), ElevenLabs empty transcript `{ text: "" }`, ElevenLabs timeout; assert correct `SttError.code` thrown in each case and successful return on happy path [owner:test-writer] [beads:pending]
- [x] [4.2] [P-1] Unit tests for `normalizeVoiceMessage()` STT wiring -- mock `bot.getFileLink()` and `transcribe()`; assert: happy path sets `text`/`content` to transcript; `transcribe()` throws `SttError` → fallback text set, no throw; `bot.getFileLink()` throws → `transcribe()` not called, file-retrieval fallback text set; all cases preserve `metadata.fileUrl` when available [owner:test-writer] [beads:pending]
