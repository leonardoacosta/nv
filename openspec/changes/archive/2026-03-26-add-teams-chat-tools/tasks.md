# Implementation Tasks

<!-- beads:epic:nv-0xm0 -->

## API Batch

- [x] [1.1] [P-1] Add `ChatInfo` struct to `crates/nv-daemon/src/channels/teams/types.rs` — fields: `id: String`, `topic: Option<String>`, `chat_type: String`, `last_updated: Option<String>`, `members: Vec<ChatMember>` where `ChatMember { display_name: String, email: Option<String> }` [owner:api-engineer]
- [x] [1.2] [P-1] Add `pub async fn list_chats(&self, limit: usize) -> Result<Vec<ChatInfo>>` to `TeamsClient` in `client.rs` — `GET /chats?$top={limit}&$expand=members&$orderby=lastMessageReceivedDateTime desc`; deserialize into `Vec<ChatInfo>`; on 403 return descriptive error about delegated vs app permissions [owner:api-engineer]
- [x] [1.3] [P-1] Add `pub async fn get_chat_messages(&self, chat_id: &str, limit: usize) -> Result<Vec<ChatMessage>>` to `TeamsClient` — `GET /chats/{chat_id}/messages?$top={limit}&$orderby=createdDateTime desc`; reuse existing `ChatMessage` type [owner:api-engineer]
- [x] [1.4] [P-2] Add `pub async fn teams_list_chats(client: &TeamsClient, limit: usize) -> Result<String>` to `tools/teams.rs` — calls `client.list_chats(limit)`, formats as table: topic/members (DMs show other person's name), type badge (DM/Group/Meeting), last activity relative time [owner:api-engineer]
- [x] [1.5] [P-2] Add `pub async fn teams_read_chat(client: &TeamsClient, chat_id: &str, limit: usize) -> Result<String>` to `tools/teams.rs` — calls `client.get_chat_messages()`, formats as message list: sender, timestamp, content (HTML stripped), truncated to 500 chars per message [owner:api-engineer]
- [x] [1.6] [P-2] Add `ToolDefinition` entries for `teams_list_chats` (input: `{"limit": "number, optional, default 20"}`) and `teams_read_chat` (input: `{"chat_id": "string, required", "limit": "number, optional, default 20"}`) to `teams_tool_definitions()` [owner:api-engineer]
- [x] [1.7] [P-2] Wire dispatch for `teams_list_chats` and `teams_read_chat` in `tools/mod.rs` — match tool name, call function, return result [owner:api-engineer]
- [x] [1.8] [P-3] Update `config/system-prompt.md` — add `teams_list_chats` and `teams_read_chat` to "Reads (immediate)" tools list [owner:api-engineer]

## Verify

- [x] [2.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [x] [2.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [x] [2.3] Unit test: `teams_list_chats` formats DM with other person's name as topic [owner:api-engineer]
- [x] [2.4] Unit test: `teams_read_chat` strips HTML from message content [owner:api-engineer]
- [x] [2.5] Unit test: tool definitions include both new tools in registry [owner:api-engineer]
- [ ] [2.6] [user] Manual test: ask Nova "list my recent Teams chats" — verify DMs and group chats appear [owner:api-engineer]
- [ ] [2.7] [user] Manual test: ask Nova "read messages from chat X" — verify message content renders [owner:api-engineer]
