# Proposal: Add Deploy Hooks

## Change ID
`add-deploy-hooks`

## Summary
Add pre-push and post-merge git hooks that trigger non-blocking deployment (build + restart)
with TTS notification on success/failure and journalctl log dump on error.

## Context
- Extends: `deploy/install.sh` (existing idempotent deploy script), `.git/hooks/`
- Related: cl, co, cw projects use the same pre-push deploy pattern

## Motivation
Currently deploying nv requires manually running `deploy/install.sh`. Every `git push` to main
should auto-deploy (build, install binaries, restart systemd services). On homelab pull
(`git pull`), post-merge should also trigger deploy. Both hooks must be non-blocking to avoid
stalling git operations — deploy runs in background with TTS notification on outcome.

## Requirements

### Req-1: Pre-push hook triggers deploy on main
A pre-push hook that detects pushes to the `main` branch and triggers `deploy/install.sh`
in the background. Skippable with `SKIP_DEPLOY=1`.

### Req-2: Post-merge hook triggers deploy
A post-merge hook that triggers `deploy/install.sh` in the background after a merge/pull
completes (e.g., when homelab pulls latest from main).

### Req-3: Non-blocking execution with TTS feedback
Both hooks run deploy in background. On completion:
- Success: TTS notification "nv deployed successfully"
- Failure: TTS notification "nv deploy failed — check logs", plus dump last 30 lines
  of `journalctl --user -u nv` to a log file for review

### Req-4: SKIP_DEPLOY bypass
`SKIP_DEPLOY=1 git push` skips the deploy hook entirely.

## Scope
- **IN**: pre-push hook, post-merge deploy trigger, background execution, TTS notification, error logging
- **OUT**: CI/CD pipeline, remote deploy, Docker, Vercel. Relay service management (already handled by install.sh)

## Impact
| Area | Change |
|------|--------|
| `.git/hooks/pre-push` | New hook file |
| `.git/hooks/post-merge` | Extend existing (currently beads-only) |
| `deploy/` | No changes to install.sh itself |

## Risks
| Risk | Mitigation |
|------|-----------|
| Build failure blocks push (if not backgrounded) | Deploy runs fully in background — git operation completes immediately |
| Post-merge conflicts with existing beads hook | Append deploy trigger after existing beads logic |
| Cargo build takes ~30s | Non-blocking — user gets TTS when done |
