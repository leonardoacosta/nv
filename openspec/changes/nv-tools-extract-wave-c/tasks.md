# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `nv-tools-extract-wave-b`

## API Batch: Tool Extraction

- [ ] [1.1] [P-1] Add `base64`, `jsonwebtoken` to `crates/nv-tools/Cargo.toml` for calendar JWT auth [owner:api-engineer]

### docker
- [ ] [2.1] [P-1] Move `docker.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### github
- [ ] [3.1] [P-1] Move `github.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### web
- [ ] [4.1] [P-1] Move `web.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### calendar
- [ ] [5.1] [P-1] Move `calendar.rs` to nv-tools, update imports (add base64 + jsonwebtoken), re-export, register [owner:api-engineer]

### tailscale
- [ ] [6.1] [P-1] Investigate `crates/nv-daemon/src/tailscale.rs` -- if purely process-shelling (Command::new), move to nv-tools; if daemon-coupled, defer to SharedDeps spec and skip [owner:api-engineer]
- [ ] [6.2] [P-2] If moved: update imports, re-export, register. If deferred: document in SharedDeps proposal [owner:api-engineer]

## Verify

- [ ] [7.1] `cargo test -p nv-daemon --lib` -- all 1,032 tests pass [owner:api-engineer]
- [ ] [7.2] `cargo build -p nv-tools` -- compiles with all 17 tool modules [owner:api-engineer]
- [ ] [7.3] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
