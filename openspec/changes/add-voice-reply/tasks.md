# Implementation Tasks

<!-- beads:epic:nv-53k -->

## DB Batch

(no database changes required)

## API Batch

- [ ] [2.1] [P-1] Add voice config fields to DaemonConfig: voice_enabled (bool, default false), voice_max_chars (u32, default 500), elevenlabs_voice_id (Option<String>), elevenlabs_model (Option<String>, default "eleven_multilingual_v2") [owner:api-engineer] [beads:nv-0i9]
- [ ] [2.2] [P-1] Add ELEVENLABS_API_KEY to Secrets struct in config.rs [owner:api-engineer] [beads:nv-dqe]
- [ ] [2.3] [P-1] Create crates/nv-daemon/src/tts.rs: ElevenLabs HTTP client (POST /v1/text-to-speech/{voice_id}, returns MP3 bytes) with 10s timeout and error handling [owner:api-engineer] [beads:nv-25q]
- [ ] [2.4] [P-1] Add ffmpeg OGG/Opus transcoding to tts.rs: spawn `ffmpeg -i pipe:0 -c:a libopus -f ogg pipe:1`, pipe MP3 stdin, collect OGG stdout [owner:api-engineer] [beads:nv-plb]
- [ ] [2.5] [P-1] Add synthesize() public function to tts.rs that chains ElevenLabs call → ffmpeg transcode → returns Result<Vec<u8>> of OGG/Opus bytes [owner:api-engineer] [beads:nv-dno]
- [ ] [2.6] [P-1] Add send_voice(chat_id, ogg_bytes, reply_to) to TelegramClient: multipart/form-data POST to sendVoice with voice field (filename "voice.ogg", content-type "audio/ogg") [owner:api-engineer] [beads:nv-vvz]
- [ ] [2.7] [P-2] Add ffmpeg availability check at daemon startup: if voice_enabled but ffmpeg not in PATH, log warning and force-disable voice [owner:api-engineer] [beads:nv-mt1]
- [ ] [2.8] [P-2] Wire voice AtomicBool in main.rs: initialize from config, pass Arc to agent loop [owner:api-engineer] [beads:nv-boj]
- [ ] [2.9] [P-2] Handle /voice Telegram command in poll loop: toggle AtomicBool, send confirmation message [owner:api-engineer] [beads:nv-d60]
- [ ] [2.10] [P-2] Add voice delivery to agent.rs: after text reply succeeds, check voice_enabled + char threshold → spawn async task for synthesize() + send_voice(), log errors without affecting text delivery [owner:api-engineer] [beads:nv-otm]
- [ ] [2.11] [P-2] Update nv.toml test fixtures in config.rs tests to cover voice fields [owner:api-engineer] [beads:nv-yut]

## UI Batch

(no UI changes — Telegram-native voice bubbles)

## E2E Batch

- [ ] [4.1] Add unit tests for tts.rs: mock ElevenLabs response, verify ffmpeg invocation, test error paths [owner:test-writer] [beads:nv-m58]
- [ ] [4.2] Add unit test for TelegramClient::send_voice: verify multipart form construction [owner:test-writer] [beads:nv-rza]
- [ ] [4.3] Add integration test for voice toggle: verify AtomicBool flips on /voice command [owner:test-writer] [beads:nv-025]
