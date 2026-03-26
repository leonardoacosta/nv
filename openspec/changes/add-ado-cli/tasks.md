# Implementation Tasks

<!-- beads:epic:nv-ztq3 -->

## API Batch

- [x] [2.1] [P-1] Add `crates/ado-cli` to workspace members in root `Cargo.toml` [owner:api-engineer]
- [x] [2.2] [P-1] Create `crates/ado-cli/Cargo.toml` with deps: nv-tools, clap, tokio, serde_json, anyhow [owner:api-engineer]
- [x] [2.3] [P-1] Re-export `AdoClient`, `AdoPipeline`, `AdoBuild`, `AdoWorkItem` and `relative_time` from `nv-tools` lib so `ado-cli` can depend on them [owner:api-engineer]
- [x] [2.4] [P-1] Implement `src/main.rs` with clap `Commands` enum: pipelines, builds, work-items, run-pipeline, plus global `--json` flag [owner:api-engineer]
- [x] [2.5] [P-1] Implement `pipelines` subcommand — call `AdoClient::pipelines`, print table or JSON [owner:api-engineer]
- [x] [2.6] [P-1] Implement `builds` subcommand — call `AdoClient::builds` (no pipeline filter, top 20), print table or JSON [owner:api-engineer]
- [x] [2.7] [P-1] Implement `work-items` subcommand with optional `--assigned-to` flag — construct WIQL, call `AdoClient::work_items_by_wiql`, print table or JSON [owner:api-engineer]
- [x] [2.8] [P-2] Implement `run-pipeline` subcommand — POST to pipelines runs API, print run ID and URL or surfaced error [owner:api-engineer]
- [x] [2.9] [P-2] Add `--json` flag propagation through all subcommands and ensure errors go to stderr regardless of flag [owner:api-engineer]
- [x] [2.10] [P-2] Verify `cargo build -p ado-cli` produces working `ado` binary; smoke-test all four subcommands against a real ADO org [owner:api-engineer]
