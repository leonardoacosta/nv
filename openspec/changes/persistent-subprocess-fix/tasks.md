# Implementation Tasks

<!-- beads:epic:nv-6leo -->

## API Batch

- [ ] [2.1] Add `#[tokio::test] #[ignore]` smoke test `persistent_subprocess_smoke` that spawns a
  real `claude` subprocess (stream-json, `--no-mcp`, no `--tools-json`), sends `"Say exactly:
  pong"`, and asserts a `result` event arrives within 15s containing "pong" — must fail before
  fixes are applied [owner:api-engineer]
- [ ] [2.2] Remove `tools_json: Option<String>` field from `SpawnConfig`; remove the block in
  `spawn_persistent` that appends `--tools-json` and the serialized tools JSON to `base_args`
  [owner:api-engineer]
- [ ] [2.3] Remove `PersistentSession::new` parameter logic that serializes tools into
  `config.tools_json`; remove the tool-change detection block in `send_turn` (the `caller_json` /
  `spawn_json` comparison block that kills and respawns on tool list changes) [owner:api-engineer]
- [ ] [2.4] Add `"--no-mcp".into()` to `base_args` in `spawn_persistent`, immediately after the
  `--no-session-persistence` entry; log a `tracing::debug!` line confirming the flag is present
  [owner:api-engineer]
- [ ] [2.5] In `drain_init_events`, change `Duration::from_secs(10)` to `Duration::from_secs(20)`;
  update the `tracing::warn!` message to say "20s timeout" [owner:api-engineer]
- [ ] [2.6] Verify the smoke test from [2.1] passes against the development `claude` binary with
  fixes from [2.2]–[2.5] applied (`cargo test -p nv-daemon -- persistent_subprocess_smoke
  --ignored`); document the observed root cause in a code comment at the top of `spawn_persistent`
  [owner:api-engineer]
- [ ] [2.7] In `PersistentSession::new`, change `fallback_only: true` to `fallback_only: false`;
  replace the disable comment with a one-line note: "Persistent mode active — root cause fixed in
  persistent-subprocess-fix spec" [owner:api-engineer]
- [ ] [2.8] Add unit test `fallback_reset_after_cooldown`: construct `SessionInner` directly with
  `fallback_only: true` and `last_failure_at = Some(Instant::now() - FALLBACK_RESET_DURATION -
  Duration::from_secs(1))`; call `ensure_alive` and assert it clears `fallback_only` (returns
  `true` or proceeds past the early-return guard) [owner:api-engineer]
- [ ] [2.9] Add unit test `fallback_no_reset_within_cooldown`: same setup but
  `last_failure_at = Some(Instant::now())`; assert `ensure_alive` returns `false` immediately
  [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [ ] [3.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [3.3] `cargo test -p nv-daemon -- persistent_subprocess_smoke --ignored` passes (requires
  live `claude` binary on dev machine with valid OAuth session) [owner:api-engineer]
- [ ] [3.4] `cargo test -p nv-daemon` passes (all non-ignored tests, including new FALLBACK_RESET
  tests and existing stream-json unit tests) [owner:api-engineer]
- [ ] [3.5] Daemon restarts cleanly after change (`systemctl restart nv-daemon`); first turn
  completes in under 5s (persistent mode active, no cold-start spawn) [owner:api-engineer]
