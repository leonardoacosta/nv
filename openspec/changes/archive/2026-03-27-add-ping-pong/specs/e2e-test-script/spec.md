# E2E Test Script

## ADDED Requirements

### Requirement: Ping health check script

A bash script at `packages/e2e/ping-health-check.sh` MUST send "ping" via the Telegram Bot API, poll for the "pong" response, and exit 0 on success or 1 on failure. It SHALL read `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` from environment variables.

#### Scenario: Daemon is healthy and responds
Given the daemon is running and processing Telegram messages
When the script sends "ping" via Bot API
Then it polls getUpdates for a "pong" response
And exits 0 within 30 seconds

#### Scenario: Daemon is down or unresponsive
Given the daemon is not running
When the script sends "ping" via Bot API
Then it polls for 30 seconds without receiving "pong"
And exits 1 with an error message

#### Scenario: Missing environment variables
Given TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID is not set
When the script is invoked
Then it exits 1 with a usage message before making any API calls

### Requirement: Deploy hook integration

The pre-push deploy hook (`deploy/pre-push.sh`) MUST call the ping health check script after daemon restart, replacing or supplementing the existing systemctl-only health check.

#### Scenario: Daemon restart triggers ping check
Given the pre-push hook has restarted the daemon
When the post-deploy health check runs
Then it calls `packages/e2e/ping-health-check.sh`
And reports the result in the deploy summary

#### Scenario: Ping check fails after deploy
Given the daemon restarted but ping check returns exit code 1
When the deploy summary is printed
Then it shows "daemon: DEGRADED (ping failed)" instead of "OK"
And the push still proceeds (non-blocking warning)
