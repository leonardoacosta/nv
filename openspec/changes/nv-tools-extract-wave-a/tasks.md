# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `nv-tools-scaffold` (ToolDefinition must be in nv-core, nv-tools crate must exist)

## API Batch: Tool Extraction

- [ ] [1.1] [P-1] Add `reqwest` workspace dependency to `crates/nv-tools/Cargo.toml` [owner:api-engineer]
- [ ] [1.2] [P-1] Create `crates/nv-tools/src/tools/mod.rs` with pub module declarations [owner:api-engineer]

### stripe
- [ ] [2.1] [P-1] Move `crates/nv-daemon/src/tools/stripe.rs` to `crates/nv-tools/src/tools/stripe.rs` [owner:api-engineer]
- [ ] [2.2] [P-1] Update imports: `use nv_core::ToolDefinition` (replace `use crate::claude::ToolDefinition`) [owner:api-engineer]
- [ ] [2.3] [P-2] Add `pub use nv_tools::tools::stripe::*` re-export in daemon's `tools/mod.rs` [owner:api-engineer]
- [ ] [2.4] [P-2] Register stripe tool definitions in nv-tools registry [owner:api-engineer]

### vercel
- [ ] [3.1] [P-1] Move `vercel.rs` to nv-tools, update imports [owner:api-engineer]
- [ ] [3.2] [P-2] Add re-export in daemon + register in nv-tools registry [owner:api-engineer]

### sentry
- [ ] [4.1] [P-1] Move `sentry.rs` to nv-tools, update imports [owner:api-engineer]
- [ ] [4.2] [P-2] Add re-export in daemon + register in nv-tools registry [owner:api-engineer]

### resend
- [ ] [5.1] [P-1] Move `resend.rs` to nv-tools, update imports [owner:api-engineer]
- [ ] [5.2] [P-2] Add re-export in daemon + register in nv-tools registry [owner:api-engineer]

### upstash
- [ ] [6.1] [P-1] Move `upstash.rs` to nv-tools, update imports [owner:api-engineer]
- [ ] [6.2] [P-2] Add re-export in daemon + register in nv-tools registry [owner:api-engineer]

## Verify

- [ ] [7.1] `cargo test -p nv-daemon --lib` -- all 1,032 tests pass (re-exports keep everything working) [owner:api-engineer]
- [ ] [7.2] `cargo build -p nv-tools` -- compiles with all 5 tool modules [owner:api-engineer]
- [ ] [7.3] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
