# Capability: Voice Toggle

## ADDED Requirements

### Requirement: Config-Based Toggle
`DaemonConfig` SHALL gain four new fields: `voice_enabled` (bool, default false), `voice_max_chars` (u32, default 500), `elevenlabs_voice_id` (Option<String>), and `elevenlabs_model` (Option<String>, default "eleven_multilingual_v2").

#### Scenario: Voice enabled in config
Given `nv.toml` contains `[daemon] voice_enabled = true`
When the daemon starts
Then voice reply capability is active (subject to ffmpeg availability check).

#### Scenario: Voice not configured
Given `nv.toml` has no voice fields under `[daemon]`
When the daemon starts
Then voice is disabled by default. No TTS calls are made.

### Requirement: Runtime Toggle via Telegram
A `/voice` command sent in the authorized Telegram chat SHALL toggle voice on/off for the current session. The toggle MUST be an `AtomicBool` initialized from the config value. Toggling MUST send a confirmation message ("Voice replies enabled" / "Voice replies disabled").

#### Scenario: Toggle voice on
Given voice is currently disabled (runtime state)
When the user sends `/voice` in Telegram
Then the AtomicBool flips to true and a confirmation message is sent.

#### Scenario: Toggle voice off
Given voice is currently enabled (runtime state)
When the user sends `/voice` in Telegram
Then the AtomicBool flips to false and a confirmation message is sent.

#### Scenario: Restart resets toggle
Given voice was toggled at runtime to a state different from config
When the daemon restarts
Then the runtime state resets to the config value.
