# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `nv-tools-extract-wave-a`

## API Batch: Tool Extraction

- [ ] [1.1] [P-1] Add `tokio-postgres`, `tokio-postgres-rustls`, `rustls` to `crates/nv-tools/Cargo.toml` for neon support [owner:api-engineer]

### ha
- [ ] [2.1] [P-1] Move `ha.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### ado
- [ ] [3.1] [P-1] Move `ado.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### plaid
- [ ] [4.1] [P-1] Move `plaid.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### doppler
- [ ] [5.1] [P-1] Move `doppler.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### cloudflare
- [ ] [6.1] [P-1] Move `cloudflare.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### posthog
- [ ] [7.1] [P-1] Move `posthog.rs` to nv-tools, update imports, re-export, register [owner:api-engineer]

### neon
- [ ] [8.1] [P-1] Move `neon.rs` to nv-tools, update imports (nv_core::ToolDefinition + tokio-postgres), re-export, register [owner:api-engineer]

## Verify

- [ ] [9.1] `cargo test -p nv-daemon --lib` -- all 1,032 tests pass [owner:api-engineer]
- [ ] [9.2] `cargo build -p nv-tools` -- compiles with all 12 tool modules (5 from wave-a + 7 new) [owner:api-engineer]
- [ ] [9.3] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
