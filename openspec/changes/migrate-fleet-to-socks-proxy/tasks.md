# Implementation Tasks

## Phase 1: Shared Infrastructure (graph-svc)

- [x] [1.1] Create `packages/tools/graph-svc/src/socks-client.ts` -- SOCKS5 HTTP client using `curl --socks5-hostname localhost:1080` via `execFile`. Export `socksGet(url, token, timeoutMs?)`, `socksPost(url, token, body, timeoutMs?)`, `socksPatch(url, token, body, timeoutMs?)`, and `isSocksAvailable()` to check proxy connectivity. Throws typed `SocksError` on failure. [owner:api-engineer]
- [x] [1.2] Create `packages/tools/graph-svc/src/token-cache.ts` -- O365 token cache. Export `getO365Token(cloudpcHost)` that reads `.graph-token.json` from CloudPC via SSH on first call, caches in memory with TTL. Export `clearO365TokenCache()` for 401 retry. ADO tokens already handled by `ado-rest.ts`. [owner:api-engineer]

## Phase 2: Rewrite graph-svc tools

- [x] [2.1] Rewrite `packages/tools/graph-svc/src/tools/calendar.ts` -- Replace `sshCloudPC()` with `socksGet()` calling Graph API: `calendarToday` -> GET `/me/calendarView?startDateTime=...&endDateTime=...`, `calendarUpcoming` -> same with N days range, `calendarNext` -> same with `$top=1&$orderby=start/dateTime`. Format JSON response as text. Fall back to SSH if SOCKS unavailable. [owner:api-engineer]
- [x] [2.2] Rewrite `packages/tools/graph-svc/src/tools/mail.ts` -- Replace `sshCloudPC()` with Graph API via SOCKS: `outlookInbox` -> GET `/me/mailFolders/Inbox/messages?$top=N`, `outlookRead` -> GET `/me/messages/{id}`, `outlookSearch` -> GET `/me/messages?$search="query"`, `outlookUnread` -> GET `/me/messages?$filter=isRead eq false`, `outlookSent` -> GET `/me/mailFolders/SentItems/messages`, `outlookFolders` -> GET `/me/mailFolders`, `outlookFolder` -> GET `/me/mailFolders/{id}/messages`, `outlookFlag` -> PATCH `/me/messages/{id}` with flag body, `outlookMove` -> POST `/me/messages/{id}/move` with destinationId body. Fall back to SSH. [owner:api-engineer]
- [x] [2.3] Rewrite `packages/tools/graph-svc/src/tools/ado.ts` -- Replace `sshAdoCommand()` with `socksGet()` using ADO REST API URLs. `adoProjects` -> GET `_apis/projects`, `adoPipelines` -> GET `{project}/_apis/pipelines`, `adoBuilds` -> GET `{project}/_apis/build/builds?$top=N`. Use ADO token from `ado-rest.ts` `getAdoToken()`. Fall back to SSH. [owner:api-engineer]
- [x] [2.4] Rewrite `packages/tools/graph-svc/src/tools/ado-extended.ts` -- Replace all `sshAdoCommand()` calls with `socksGet()`/`socksPost()` using ADO REST API: work items via WIQL POST, repos list, PRs list, build logs, commits, pipeline definitions, pipeline update (PATCH/PUT), repo update, pipeline run (POST), pipeline variables, branches, repo delete. Use ADO token from `ado-rest.ts`. Fall back to SSH. [owner:api-engineer]
- [x] [2.5] Rewrite `packages/tools/graph-svc/src/tools/pim.ts` -- Replace `sshCloudPC()` with SOCKS call to Azure PIM API: `pimStatus` -> GET `/providers/Microsoft.Authorization/roleEligibilityScheduleInstances?$filter=asTarget()`, `pimActivate`/`pimActivateAll` -> POST role activation. Uses management.azure.com token. Fall back to SSH. [owner:api-engineer]

## Phase 3: Teams service migration

- [x] [3.1] Create `packages/tools/teams-svc/src/socks-client.ts` -- Same SOCKS client pattern as graph-svc. [owner:api-engineer]
- [x] [3.2] Create `packages/tools/teams-svc/src/token-cache.ts` -- O365 token cache, same pattern as graph-svc. [owner:api-engineer]
- [x] [3.3] Rewrite `packages/tools/teams-svc/src/tools/list-chats.ts` -- Replace `sshCloudPc()` with Graph API: GET `/me/chats?$expand=lastMessagePreview&$top=N`. Fall back to SSH. [owner:api-engineer]
- [x] [3.4] Rewrite `packages/tools/teams-svc/src/tools/read-chat.ts` -- Replace with GET `/me/chats/{chatId}/messages?$top=N`. Fall back to SSH. [owner:api-engineer]
- [x] [3.5] Rewrite `packages/tools/teams-svc/src/tools/messages.ts` -- Replace with GET `/teams/{teamId}/channels/{channelId}/messages?$top=N`. Resolve team/channel names to IDs via Graph API first. Fall back to SSH. [owner:api-engineer]
- [x] [3.6] Rewrite `packages/tools/teams-svc/src/tools/channels.ts` -- Replace with GET `/me/joinedTeams` then GET `/teams/{id}/channels`. Fall back to SSH. [owner:api-engineer]
- [x] [3.7] Rewrite `packages/tools/teams-svc/src/tools/presence.ts` -- Replace with GET `/users/{user}/presence`. Fall back to SSH. [owner:api-engineer]
- [x] [3.8] Rewrite `packages/tools/teams-svc/src/tools/send.ts` -- Replace with POST `/me/chats/{chatId}/messages` with body `{body:{content:message}}`. Fall back to SSH. [owner:api-engineer]

## Phase 4: Validation

- [x] [4.1] Run `pnpm --filter @nova/graph-svc typecheck` -- must pass. [owner:api-engineer]
- [x] [4.2] Run `pnpm --filter @nova/teams-svc typecheck` -- must pass. [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Infrastructure | `pnpm --filter @nova/graph-svc typecheck` passes with new files |
| 2 Graph tools | Typecheck passes with rewritten tools |
| 3 Teams tools | `pnpm --filter @nova/teams-svc typecheck` passes |
| **Final** | Both services typecheck clean. SOCKS path works when proxy is up. SSH fallback works when proxy is down. |
