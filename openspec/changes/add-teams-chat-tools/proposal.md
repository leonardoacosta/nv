# Proposal: Add Teams Chat/DM Reading Tools

## Change ID
`add-teams-chat-tools`

## Summary

Add two new Teams tools — `teams_list_chats` and `teams_read_chat` — so Nova can read DMs and group chat messages, not just channel messages. The Graph API permissions (Chat.Read) are already granted.

## Context
- Extends: `crates/nv-daemon/src/tools/teams.rs` (4 existing tools), `crates/nv-daemon/src/channels/teams/client.rs` (TeamsClient)
- Related: existing `teams_messages` reads channel messages only; `send_chat_message` already uses `/chats/{chat_id}/messages` endpoint for writes
- Graph API: `GET /me/chats` (list chats), `GET /chats/{chat_id}/messages` (read messages)

## Motivation

Nova can currently read Teams channel messages via `teams_messages` but cannot read DMs or group chats. The Graph API permission Chat.Read is already configured on the Azure AD app (evidenced by `send_chat_message` writing to `/chats/{chat_id}/messages`). The read endpoints use the same permission scope. This gap means Nova can't see the actual decision threads and cross-team DMs where most work communication happens.

## Requirements

### Req-1: TeamsClient — list_chats

Add `pub async fn list_chats(&self, limit: usize) -> Result<Vec<ChatInfo>>` to TeamsClient.

- Calls `GET /me/chats?$top={limit}&$expand=members&$orderby=lastMessageReceivedDateTime desc`
- Note: `/me/chats` requires **delegated** permissions. If the current auth is app-only (client credentials), this endpoint returns 403. In that case, use `GET /chats` with `Chat.Read.All` application permission, or fall back gracefully with a clear error message.
- Returns `Vec<ChatInfo>` where `ChatInfo { id, topic, chat_type, last_updated, members: Vec<String> }`
- `chat_type` maps: `oneOnOne` = "DM", `group` = "Group", `meeting` = "Meeting"

### Req-2: TeamsClient — get_chat_messages

Add `pub async fn get_chat_messages(&self, chat_id: &str, limit: usize) -> Result<Vec<ChatMessage>>` to TeamsClient.

- Calls `GET /chats/{chat_id}/messages?$top={limit}&$orderby=createdDateTime desc`
- Reuses the existing `ChatMessage` type (already defined in `types.rs`)
- Returns messages newest-first, limit default 20, max 50

### Req-3: teams_list_chats tool

Register `teams_list_chats` tool in `teams_tool_definitions()`.

- Input: `{ "limit": 20 }` (optional, default 20, max 50)
- Output: formatted table of chats — topic/members, type, last activity
- For DMs: show the other person's name as the "topic" (since DMs have no topic)

### Req-4: teams_read_chat tool

Register `teams_read_chat` tool in `teams_tool_definitions()`.

- Input: `{ "chat_id": "required", "limit": 20 }` (limit optional, default 20)
- Output: formatted message list — sender, timestamp, content (HTML stripped to plain text)
- Reuse `strip_html` or similar from the existing `teams_messages` formatter

### Req-5: System prompt update

Add `teams_list_chats` and `teams_read_chat` to the "Reads (immediate)" tool list in `config/system-prompt.md`.

## Scope
- **IN**: list_chats client method, get_chat_messages client method, two tool definitions, tool dispatch, system prompt update
- **OUT**: chat search/filtering, message reactions, file attachments in chats, creating new chats

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/teams/client.rs` | Add `list_chats`, `get_chat_messages` methods |
| `crates/nv-daemon/src/channels/teams/types.rs` | Add `ChatInfo` struct |
| `crates/nv-daemon/src/tools/teams.rs` | Add 2 tool definitions + 2 async dispatch functions |
| `crates/nv-daemon/src/tools/mod.rs` | Wire dispatch for new tools |
| `config/system-prompt.md` | Add tool names to reads list |

## Risks

| Risk | Mitigation |
|------|-----------|
| `/me/chats` requires delegated auth, not app-only | Detect 403 and return helpful error; document which auth mode is needed |
| Chat messages may contain HTML | Reuse existing HTML stripping from teams_messages |
| Large chat lists | Default limit 20, max 50 |
