# Implementation Tasks

<!-- beads:epic:nv-18f -->

## Infra Batch

- [x] [1.1] [P-1] Create `doppler.yaml` at repo root mapping project=nova, config=prd [owner:devops-engineer] [beads:nv-l0a]
- [x] [1.2] [P-1] Update `deploy/nv.service`: replace `EnvironmentFile` with `doppler run --fallback=true --` in `ExecStart` [owner:devops-engineer] [beads:nv-tsr]
- [x] [1.3] [P-1] Update `relays/teams/nv-teams-relay.service`: same pattern as 1.2 [owner:devops-engineer] [beads:nv-9di]
- [x] [1.4] [P-1] Update `relays/discord/nv-discord-relay.service`: same pattern as 1.2 [owner:devops-engineer] [beads:nv-cus]
- [x] [1.5] [P-2] Update `config/env.example` with Doppler migration note and secret inventory reference [owner:devops-engineer] [beads:nv-ril]

## Code Batch

- [x] [2.1] [P-1] Remove manual env file parser from `crates/nv-cli/src/main.rs` (lines 63-85) [owner:api-engineer] [beads:nv-p6n]

## User Batch

- [x] [3.1] [P-1] Install Doppler CLI on homelab — verified v3.75.1 [owner:devops-engineer]
- [x] [3.2] [P-1] Create Doppler project `nova` with `prd` config [owner:devops-engineer]
- [x] [3.3] [P-1] Populate all secrets from `config/env` into Doppler `nova/prd` via `doppler secrets set` [owner:devops-engineer]
- [x] [3.4] [P-1] Configure Doppler service token for systemd [owner:devops-engineer]
- [ ] [3.5] [P-2] Reload systemd units and restart services [owner:devops-engineer]
- [ ] [3.6] [P-2] Verify all services running [owner:devops-engineer]
- [ ] [3.7] [P-3] Remove `~/.nv/env` once migration verified (keep backup) [owner:devops-engineer]
