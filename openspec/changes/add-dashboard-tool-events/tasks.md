# Implementation Tasks

## API Batch

- [ ] [1.1] [P-2] In packages/daemon/src/http.ts: import `humanizeToolName` from `../channels/tool-names.js`, add `else if (event.type === "tool_start")` branch that emits `{ type: "tool_start", name: humanizeToolName(event.name), callId: event.callId }` via `stream.writeSSE`, add `else if (event.type === "tool_done")` branch that emits `{ type: "tool_done", name: humanizeToolName(event.name), callId: event.callId, durationMs: event.durationMs }` via `stream.writeSSE`, and remove the `// tool_start and tool_done are ignored in SSE output for now` comment [owner:api-engineer]
- [ ] [1.2] [P-2] Build verification -- run `pnpm tsc --noEmit` from packages/daemon to confirm no type errors after the changes [owner:api-engineer]
