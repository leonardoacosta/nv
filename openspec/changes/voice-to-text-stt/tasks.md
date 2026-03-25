# Tasks: voice-to-text-stt

<!-- beads:epic:nv-8jqa -->

## Deepgram STT Function

- [ ] [1.1] [P-1] Add `transcribe_audio_deepgram(audio_bytes, mime_type, api_key, model)` to `crates/nv-daemon/src/speech_to_text.rs` — POST raw bytes to `https://api.deepgram.com/v1/listen?model={model}&smart_format=true`, extract `results.channels[0].alternatives[0].transcript`, return `Err` on non-2xx, parse failure, missing path, or empty transcript [owner:api-engineer]
- [ ] [1.2] [P-2] Add unit tests in `speech_to_text.rs` for Deepgram response parsing: success case, missing `alternatives` path, empty transcript, HTTP error body [owner:api-engineer]

## Large-File Gate

- [ ] [2.1] [P-1] In `transcribe_voice_message()` (`crates/nv-daemon/src/channels/telegram/mod.rs`): read `file_size` from `msg.metadata`, reject with Telegram reply `"Voice message too large to transcribe (max 20 MB)."` if `file_size > 20_971_520`; skip gate when field is absent [owner:api-engineer]

## Switch Voice Path to Deepgram

- [ ] [3.1] [P-1] Update `transcribe_voice_message()` to read `DEEPGRAM_API_KEY` env var (replacing `ELEVENLABS_API_KEY`); send `"Voice transcription not configured (DEEPGRAM_API_KEY missing)."` reply when unset [owner:api-engineer]
- [ ] [3.2] [P-1] Replace `transcribe_audio_elevenlabs(...)` call with `transcribe_audio_deepgram(audio_bytes, mime_type, &api_key, &model)` in `transcribe_voice_message()` [owner:api-engineer]
- [ ] [3.3] [P-2] Add `model: String` parameter to `transcribe_voice_message()` and `run_poll_loop()`; thread `config.agent.deepgram_model` through from call sites in `main.rs` / orchestrator startup [owner:api-engineer]

## Config

- [ ] [4.1] [P-2] Add `deepgram_model: String` field to `AgentConfig` in `crates/nv-core/src/config.rs` with `#[serde(default = "default_deepgram_model")]` and `fn default_deepgram_model() -> String { "nova-2".to_string() }` [owner:api-engineer]

## Doppler

- [ ] [5.1] [P-1] Add `DEEPGRAM_API_KEY` to Doppler nv-daemon config (dev + prod environments) [user]

## Verify

- [ ] [6.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [6.2] `cargo clippy -- -D warnings` passes with no warnings [owner:api-engineer]
- [ ] [6.3] `cargo test -p nv-daemon` — all existing tests pass, new Deepgram parse tests pass [owner:api-engineer]
- [ ] [6.4] Manual gate: send a Telegram voice message, verify it is transcribed and dispatched to the agent loop [user]
- [ ] [6.5] Manual gate: send a voice message >20 MB (or simulate via metadata), verify the "too large" reply is sent and no download is attempted [user]
