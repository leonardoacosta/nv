# Proposal: Entity Resolution Layer

## Change ID
`entity-resolution-layer`

## Summary

Build a unified entity resolution API layer that cross-references contacts, projects, obligations, messages, sessions, and memory to break the dashboard's data silos. Given any entity (contact, project, obligation), the API returns all related entities across tables. Contacts are enriched with resolved names from memory's `people` namespace (replacing raw Telegram IDs like "7380462766"). Projects are enriched with memory's `projects-*` namespaces. New API routes return related entities: messages for a contact, obligations for a project, sessions for a project.

## Context
- Depends on: `rewire-dashboard-api` (completed -- dashboard uses Drizzle directly via `@nova/db`)
- Conflicts with: none (additive API routes + shared resolution library)
- Current state: All 9 DB tables (`messages`, `obligations`, `contacts`, `diary`, `memory`, `briefings`, `reminders`, `schedules`, `sessions`) are queryable from the dashboard via Drizzle. However, each API route and page queries exactly one table in isolation. No cross-referencing exists.
- Key pain points observed during UX exploration:
  - Contacts page shows raw sender identifiers (e.g., Telegram user ID "7380462766") because the `messages.sender` field stores the raw channel identifier, not the resolved name from memory
  - Projects page reads from `NV_PROJECTS` env var (a static JSON array with `{code, path}`), ignoring memory topics like `projects-clients`, `projects-personal`, `projects-infrastructure` that contain rich project context
  - No way to navigate from a contact to their messages, from a project to its obligations, or from an obligation to the session that executed it
  - Memory table has `people` topic with ~14KB of profile data (names, roles, channels, relationships) that is invisible to the dashboard

### Schema Reference

```
contacts:    id, name, channelIds (jsonb), relationshipType, notes, createdAt
messages:    id, channel, sender, content, metadata (jsonb), createdAt, embedding
obligations: id, detectedAction, owner, status, priority, projectCode, sourceChannel, sourceMessage, deadline, lastAttemptAt, createdAt, updatedAt
sessions:    id, project, command, status, startedAt, stoppedAt
memory:      id, topic (unique), content (text blob), embedding, updatedAt
diary:       id, triggerType, triggerSource, channel, slug, content, toolsUsed, tokensIn, tokensOut, responseLatencyMs, createdAt
reminders:   id, message, dueAt, channel, createdAt, deliveredAt, cancelled, obligationId
schedules:   id, name, cronExpr, action, channel, enabled, createdAt, lastRunAt
briefings:   id, generatedAt, content, sourcesStatus, suggestedActions
```

### Cross-Reference Opportunities

| From | To | Join Key |
|------|----|----------|
| contacts.channelIds | messages.sender + messages.channel | channelIds jsonb contains `{telegram: "7380462766"}`, messages have `sender: "7380462766"` + `channel: "telegram"` |
| contacts.name | memory.topic = "people" | Memory `people` topic contains structured profiles with channel IDs, names, roles |
| obligations.projectCode | sessions.project | Both use the same project code string (e.g., "nv") |
| obligations.sourceChannel | messages.channel | Obligation was detected from a message on that channel |
| obligations.sourceMessage | messages.content | Obligation's source_message is a substring/quote of the original message content |
| sessions.project | memory.topic LIKE "projects-%" | Memory has `projects-clients`, `projects-personal`, etc. with project context |
| reminders.obligationId | obligations.id | Direct FK relationship |

## Motivation

The dashboard presents Nova's data as disconnected lists: a contacts page, a messages page, an obligations page, a sessions page. But Nova's actual mental model is relational -- she knows that "Therese Lay" is a Teams contact who manages project Fireball, has 3 open obligations, and was last active 2 hours ago. The dashboard cannot express this because each API route queries a single table.

This creates three concrete problems:

1. **Identity fragmentation**: The contacts page shows raw channel identifiers instead of resolved names. A Telegram contact appears as "7380462766" because the messages table stores the Telegram user ID, and the dashboard has no mechanism to resolve it against the `people` memory topic or the contacts table's `channelIds` mapping.

2. **Orphaned context**: Memory topics like `projects-clients` and `projects-personal` contain rich project descriptions, client relationships, and status updates that are invisible outside the Memory page. The Projects page shows a static list from an env var. There is no way to see "what does Nova know about this project?" alongside the project's obligations and sessions.

3. **No navigation between entities**: Clicking a contact should show their messages and obligations. Clicking a project should show its sessions, obligations, and memory context. Clicking an obligation should link to the originating message. None of these paths exist.

The entity resolution layer solves all three by providing a shared resolution library and cross-reference API routes that the dashboard pages can consume.

## Requirements

### Req-1: Memory People Parser

Create `apps/dashboard/lib/entity-resolution/people-parser.ts`:

Parse the `people` memory topic content into structured profiles. The `people` topic is a text blob with entries in a semi-structured format (name, role, channels, notes separated by sections). The parser extracts:

- `name`: the person's display name
- `channelIds`: map of channel -> identifier (e.g., `{telegram: "7380462766", teams: "therese.lay@bbins.com"}`)
- `role`: role/title if mentioned
- `notes`: raw text of the person's profile section

The parser returns `PersonProfile[]` where:
```typescript
interface PersonProfile {
  name: string;
  channelIds: Record<string, string>;
  role: string | null;
  notes: string;
}
```

The parser must be tolerant of format variations -- the memory content is written by Nova in natural language and may not follow a strict schema. Use heuristic matching: look for lines containing channel identifiers (Telegram IDs are numeric, Teams IDs contain `@`, Discord IDs are numeric snowflakes), name headers (lines that start with `##` or `**name**`), and role indicators ("PM", "engineer", "manager", "lead", etc.).

### Req-2: Contact Resolver

Create `apps/dashboard/lib/entity-resolution/contact-resolver.ts`:

Given a sender identifier and channel from the messages table, resolve the display name by checking (in order):

1. **Contacts table**: Look up `contacts.channelIds` jsonb for a matching `{channel: sender}` pair. If found, return the contact's `name`.
2. **Memory people profiles**: Parse the `people` memory topic (Req-1) and find a profile whose `channelIds[channel]` matches the sender. If found, return the profile's `name`.
3. **Sender string as-is**: If no match, return the original sender string.

Export a `resolveContacts()` function that takes the full contacts list and people profiles, then returns a `Map<string, string>` mapping `"channel:sender"` -> `displayName`. This map is computed once per page load and reused for all message rendering.

```typescript
function resolveContacts(
  contacts: Contact[],
  peopleProfiles: PersonProfile[],
): Map<string, string>;  // key: "telegram:7380462766" -> value: "Leo"
```

### Req-3: Project Enrichment

Create `apps/dashboard/lib/entity-resolution/project-enrichment.ts`:

Enrich the static project list (from `NV_PROJECTS`) with data from memory and the DB:

1. **Memory enrichment**: For each project, look for memory topics matching `projects-*` that mention the project code. Parse relevant context (description, status, client, tech stack) from the topic content.
2. **Obligation count**: Count obligations grouped by `projectCode` for each project.
3. **Session count**: Count sessions grouped by `project` for each project.
4. **Last activity**: Find the most recent `sessions.startedAt` or `obligations.updatedAt` for each project.

Return an enriched project type:

```typescript
interface EnrichedProject {
  code: string;
  path: string;
  description: string | null;      // from memory
  memoryContext: string | null;     // raw memory excerpt
  obligationCount: number;
  activeObligationCount: number;    // status = 'open' or 'in_progress'
  sessionCount: number;
  lastActivity: string | null;      // ISO timestamp
}
```

### Req-4: Related Entities API -- Contact Relations

Create `apps/dashboard/app/api/contacts/[id]/related/route.ts`:

`GET /api/contacts/:id/related` returns all entities related to a contact:

- **messages**: Recent messages (limit 50) where `sender` matches any of the contact's channel identifiers from `contacts.channelIds`. Query: select from messages where (channel, sender) IN contact's channelIds pairs, ordered by createdAt desc.
- **obligations**: Obligations where `sourceChannel` matches a channel the contact is active on AND `sourceMessage` content overlaps with messages from that contact (fuzzy -- check if any message from the contact within +/- 5 minutes of the obligation's createdAt exists on the same channel).
- **memory_profile**: The section of the `people` memory topic that mentions this contact's name, extracted via the people parser.

Response shape:
```json
{
  "contact": { "id": "...", "name": "...", "channel_ids": {...} },
  "messages": [{ "id": "...", "channel": "...", "content": "...", "created_at": "..." }],
  "message_count": 847,
  "obligations": [{ "id": "...", "detected_action": "...", "status": "...", "project_code": "..." }],
  "memory_profile": "Section of people topic about this contact, or null",
  "channels_active": ["telegram", "teams"]
}
```

### Req-5: Related Entities API -- Project Relations

Create `apps/dashboard/app/api/projects/[code]/related/route.ts`:

`GET /api/projects/:code/related` returns all entities related to a project:

- **obligations**: All obligations where `projectCode` matches the project code. Ordered by updatedAt desc.
- **sessions**: All sessions where `project` matches the project code. Ordered by startedAt desc.
- **memory_topics**: All memory topics whose name starts with `projects-` and whose content mentions the project code. Return topic name + first 500 chars of content as a preview.
- **recent_messages**: Recent messages (limit 20) that mention the project code in their content (case-insensitive LIKE search).

Response shape:
```json
{
  "project": { "code": "nv", "path": "~/dev/nv" },
  "obligations": [{ "id": "...", "detected_action": "...", "status": "...", "priority": 1 }],
  "obligation_summary": { "total": 12, "open": 5, "in_progress": 2, "done": 5 },
  "sessions": [{ "id": "...", "command": "...", "status": "...", "started_at": "..." }],
  "session_count": 34,
  "memory_topics": [{ "topic": "projects-clients", "preview": "..." }],
  "recent_messages": [{ "id": "...", "channel": "...", "sender": "...", "content": "...", "created_at": "..." }]
}
```

### Req-6: Related Entities API -- Obligation Relations

Create `apps/dashboard/app/api/obligations/[id]/related/route.ts`:

`GET /api/obligations/:id/related` returns entities related to an obligation:

- **source_message**: If `sourceMessage` is set, find the closest matching message in the messages table on the same `sourceChannel` by content similarity (LIKE match on a substring of sourceMessage, within 1 hour of the obligation's createdAt).
- **project**: If `projectCode` is set, return the enriched project info (obligation count, session count, memory context).
- **reminders**: All reminders where `obligationId` matches this obligation's id.
- **related_obligations**: Other obligations with the same `projectCode` (excluding self), ordered by createdAt desc, limit 10.

Response shape:
```json
{
  "obligation": { "id": "...", "detected_action": "...", "status": "...", "project_code": "nv" },
  "source_message": { "id": "...", "channel": "telegram", "sender": "Leo", "content": "...", "created_at": "..." },
  "project": { "code": "nv", "obligation_count": 12, "session_count": 34 },
  "reminders": [{ "id": "...", "message": "...", "due_at": "...", "status": "pending" }],
  "related_obligations": [{ "id": "...", "detected_action": "...", "status": "..." }]
}
```

### Req-7: Sender Resolution API

Create `apps/dashboard/app/api/resolve/senders/route.ts`:

`GET /api/resolve/senders` returns a precomputed sender-to-name resolution map for the frontend:

1. Load all contacts from the contacts table
2. Load and parse the `people` memory topic via the people parser (Req-1)
3. Build the resolution map via `resolveContacts()` (Req-2)
4. Return the map as JSON

Response shape:
```json
{
  "resolutions": {
    "telegram:7380462766": "Leo",
    "teams:therese.lay@bbins.com": "Therese Lay",
    "discord:123456789": "Kirk"
  },
  "source_counts": {
    "contacts_table": 8,
    "memory_people": 12,
    "unresolved": 5
  }
}
```

This endpoint is called once on dashboard load and cached client-side. Pages that display sender names use this map to resolve raw identifiers to display names.

### Req-8: Projects Enrichment API

Update `apps/dashboard/app/api/projects/route.ts`:

Replace the static env-var-only response with enriched project data:

1. Read the base project list from `NV_PROJECTS` env var (existing behavior)
2. Query obligations grouped by `projectCode` (count total, count where status IN ('open', 'in_progress'))
3. Query sessions grouped by `project` (count total, max startedAt)
4. Query memory topics matching `projects-%` and extract relevant content per project
5. Return `EnrichedProject[]` (Req-3 type)

The existing `ProjectsGetResponse` type is extended to include the enrichment fields. The `ApiProject` base type remains for backward compatibility but the response includes the new fields.

### Req-9: TypeScript Types

Add to `apps/dashboard/types/api.ts`:

```typescript
// Entity resolution types
interface PersonProfile {
  name: string;
  channel_ids: Record<string, string>;
  role: string | null;
  notes: string;
}

interface SenderResolutionResponse {
  resolutions: Record<string, string>;
  source_counts: {
    contacts_table: number;
    memory_people: number;
    unresolved: number;
  };
}

interface EnrichedProject extends ApiProject {
  description: string | null;
  memory_context: string | null;
  obligation_count: number;
  active_obligation_count: number;
  session_count: number;
  last_activity: string | null;
}

interface ContactRelatedResponse {
  contact: Contact;
  messages: StoredMessage[];
  message_count: number;
  obligations: DaemonObligation[];
  memory_profile: string | null;
  channels_active: string[];
}

interface ProjectRelatedResponse {
  project: ApiProject;
  obligations: DaemonObligation[];
  obligation_summary: {
    total: number;
    open: number;
    in_progress: number;
    done: number;
  };
  sessions: NexusSessionRaw[];
  session_count: number;
  memory_topics: Array<{ topic: string; preview: string }>;
  recent_messages: StoredMessage[];
}

interface ObligationRelatedResponse {
  obligation: DaemonObligation;
  source_message: StoredMessage | null;
  project: { code: string; obligation_count: number; session_count: number } | null;
  reminders: Array<{ id: string; message: string; due_at: string; status: string }>;
  related_obligations: DaemonObligation[];
}
```

## Scope
- **IN**: People memory parser, contact name resolver, project enrichment logic, 4 new API routes (`/api/contacts/:id/related`, `/api/projects/:code/related`, `/api/obligations/:id/related`, `/api/resolve/senders`), enriched `/api/projects` response, TypeScript types for all new response shapes
- **OUT**: UI changes to dashboard pages (separate follow-up specs will consume these APIs), changes to DB schema (no new tables or columns), memory topic format changes, write operations (this layer is read-only), real-time resolution (computed per-request, no caching layer), changes to the daemon or fleet services

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/lib/entity-resolution/people-parser.ts` | NEW -- parse memory `people` topic into structured PersonProfile[] |
| `apps/dashboard/lib/entity-resolution/contact-resolver.ts` | NEW -- resolve sender identifiers to display names via contacts table + memory |
| `apps/dashboard/lib/entity-resolution/project-enrichment.ts` | NEW -- enrich static project list with obligations, sessions, memory context |
| `apps/dashboard/lib/entity-resolution/index.ts` | NEW -- barrel export |
| `apps/dashboard/app/api/contacts/[id]/related/route.ts` | NEW -- related entities for a contact (messages, obligations, memory profile) |
| `apps/dashboard/app/api/projects/[code]/related/route.ts` | NEW -- related entities for a project (obligations, sessions, memory, messages) |
| `apps/dashboard/app/api/obligations/[id]/related/route.ts` | NEW -- related entities for an obligation (source message, project, reminders, siblings) |
| `apps/dashboard/app/api/resolve/senders/route.ts` | NEW -- sender-to-name resolution map |
| `apps/dashboard/app/api/projects/route.ts` | MODIFY -- enrich response with obligation/session counts, memory context |
| `apps/dashboard/types/api.ts` | MODIFY -- add entity resolution types (PersonProfile, SenderResolutionResponse, EnrichedProject, ContactRelatedResponse, ProjectRelatedResponse, ObligationRelatedResponse) |

## Risks

| Risk | Mitigation |
|------|-----------|
| Memory `people` topic has no guaranteed format -- parsing may be fragile | The people parser uses heuristic matching (channel ID patterns, name headers, role keywords) rather than strict parsing. Unrecognized sections are returned as raw text in the `notes` field. A malformed topic degrades gracefully to showing raw sender IDs (current behavior). |
| Sender resolution adds latency to initial page load | The `/api/resolve/senders` endpoint is called once and cached client-side for the session. The resolution map is typically small (< 100 entries). Server-side computation involves 1 contacts table query + 1 memory topic read + in-memory matching -- expected < 50ms. |
| Contact `channelIds` jsonb has inconsistent format across entries | The resolver handles both `{telegram: "id"}` and `{telegram: {id: "id", name: "name"}}` formats by normalizing to string values. Missing or malformed entries are skipped. |
| Project enrichment queries (obligations + sessions + memory) may be slow with large datasets | Queries use indexed columns (`projectCode`, `project`, `topic`). Results are aggregated via COUNT/MAX in SQL, not fetched in full. Memory topic matching uses `LIKE 'projects-%'` which hits the topic unique index. For the expected scale (< 50 projects, < 1000 obligations, < 500 sessions), all queries complete in < 100ms. |
| Obligation source message matching is fuzzy and may return false positives | The match uses both content substring matching AND a time window (+/- 1 hour of obligation creation). If no confident match is found, `source_message` returns null rather than a weak match. |
| LIKE queries on `messages.content` for project mention search may be slow | Limited to 20 results with an ORDER BY on the indexed `created_at` column. For large message tables, consider adding a GIN trigram index in a follow-up spec. Current table size (< 15K messages) is within acceptable scan range. |
