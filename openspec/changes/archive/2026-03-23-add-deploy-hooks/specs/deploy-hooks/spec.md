# Deploy Hooks

## ADDED Requirements

### Requirement: Pre-push hook MUST trigger deploy on main branch pushes

The pre-push hook SHALL detect pushes targeting the `main` branch and launch
`deploy/install.sh` in the background. The git push SHALL NOT be blocked by the deploy.
`SKIP_DEPLOY=1` SHALL bypass the hook entirely.

#### Scenario: Push to main triggers background deploy
Given the user pushes to main
When the pre-push hook runs
Then deploy/install.sh is launched in the background
And the git push completes immediately without waiting

#### Scenario: Push to non-main branch skips deploy
Given the user pushes to a feature branch
When the pre-push hook runs
Then no deploy is triggered

#### Scenario: SKIP_DEPLOY bypasses hook
Given SKIP_DEPLOY=1 is set in the environment
When the user pushes to main
Then no deploy is triggered

### Requirement: Post-merge hook MUST trigger deploy after pull/merge

The post-merge hook SHALL trigger `deploy/install.sh` in the background after a
successful merge. Existing beads import logic SHALL be preserved.

#### Scenario: Git pull triggers deploy
Given the user runs git pull on main
When the post-merge hook fires
Then beads import runs first (existing behavior)
Then deploy/install.sh is launched in the background

### Requirement: Deploy outcome MUST produce TTS notification

Both hooks SHALL send a TTS notification via `claude-notify` on deploy completion.
On failure, the notification SHALL include guidance to check logs, and the hook SHALL
dump `journalctl --user -u nv -n 30` to `~/.nv/logs/deploy-error.log`.

#### Scenario: Successful deploy sends TTS
Given deploy/install.sh exits 0
Then TTS notification "nv deployed successfully" is sent

#### Scenario: Failed deploy sends TTS and logs
Given deploy/install.sh exits non-zero
Then TTS notification "nv deploy failed — check logs" is sent
And last 30 lines of journalctl are written to ~/.nv/logs/deploy-error.log
