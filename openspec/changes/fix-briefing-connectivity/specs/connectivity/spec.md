# Spec: Briefing Connectivity

## MODIFIED Requirements

### Requirement: Fix DAEMON_URL in docker-compose.yml

The `DAEMON_URL` environment variable in `docker-compose.yml` MUST use HTTP protocol on port 7700 so server-side fetch calls succeed.

#### Scenario: tRPC briefing.generate reaches daemon
Given the dashboard container is running
When the user clicks "Generate Now" and the tRPC mutation fires
Then the server-side `fetch("${DAEMON_URL}/briefing/generate")` call succeeds with HTTP 200

#### Scenario: chat/send route reaches daemon
Given the dashboard container is running
When a user sends a chat message
Then the server-side `fetch("${DAEMON_URL}/chat")` call succeeds

### Requirement: SSE stream proxied through Next.js

A new API route at `/api/briefing/stream` MUST proxy the daemon's SSE stream server-side so the browser SHALL NOT connect to the daemon directly.

#### Scenario: SSE stream proxied successfully
Given the daemon is running on port 7700
When the browser opens EventSource("/api/briefing/stream")
Then the Next.js route connects to `${DAEMON_URL}/api/briefing/stream` server-side
And forwards each SSE event (block, done, error) to the browser unchanged

#### Scenario: daemon unreachable during SSE
Given the daemon is not running
When the browser opens EventSource("/api/briefing/stream")
Then the proxy returns a 503 error
And the client falls back to the tRPC mutation path

### Requirement: Client uses relative SSE URL

The briefing page MUST connect to `/api/briefing/stream` (relative) instead of `${NEXT_PUBLIC_DAEMON_URL}/api/briefing/stream` (absolute). The `NEXT_PUBLIC_DAEMON_URL` reference MUST be removed.

#### Scenario: SSE works through Traefik
Given the user accesses the dashboard via nova.leonardoacosta.dev
When they click "Generate Now"
Then EventSource connects to `/api/briefing/stream` (same origin)
And SSE events stream through Traefik → Next.js → daemon → back

## ADDED Requirements

### Requirement: SSE proxy API route

The dashboard MUST expose a `/api/briefing/stream` GET endpoint that proxies the daemon's SSE stream server-side.

#### Scenario: proxy streams briefing blocks
Given the daemon is running
When the browser opens EventSource("/api/briefing/stream")
Then the proxy connects to `${DAEMON_URL}/api/briefing/stream` and forwards all SSE events

## REMOVED Requirements

None.
