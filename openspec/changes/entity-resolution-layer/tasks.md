# Implementation Tasks

## DB Batch: Entity Resolution Library

- [x] [1.1] [P-1] Create `apps/dashboard/lib/entity-resolution/people-parser.ts` -- export `parsePeopleMemory(content: string): PersonProfile[]`; parse the `people` memory topic text blob into structured profiles; detect name headers (lines starting with `##` or `**name**` or all-caps names), extract channel identifiers by pattern (Telegram IDs = numeric strings 6-15 digits, Teams IDs = contain `@`, Discord IDs = numeric snowflakes 17-20 digits), extract role keywords (PM, engineer, manager, lead, developer, designer, etc.), return remaining text as `notes`; handle missing/malformed sections gracefully by returning partial profiles [owner:api-engineer]
- [x] [1.2] [P-1] Create `apps/dashboard/lib/entity-resolution/contact-resolver.ts` -- export `resolveContacts(contacts: Contact[], peopleProfiles: PersonProfile[]): Map<string, string>`; build a `Map<"channel:sender", displayName>` by: (1) iterating contacts table rows, for each channelId entry add `"channel:id" -> contact.name`, (2) iterating peopleProfiles, for each channelId entry add `"channel:id" -> profile.name` (only if not already mapped from contacts table, contacts table takes precedence); key format is `"${channel}:${senderId}"` [owner:api-engineer]
- [x] [1.3] [P-2] Create `apps/dashboard/lib/entity-resolution/project-enrichment.ts` -- export `enrichProjects(projects: ApiProject[], db: DrizzleClient): Promise<EnrichedProject[]>`; for each project: (1) query `SELECT COUNT(*) FROM obligations WHERE project_code = ?` and `SELECT COUNT(*) FROM obligations WHERE project_code = ? AND status IN ('open', 'in_progress')`, (2) query `SELECT COUNT(*), MAX(started_at) FROM sessions WHERE project = ?`, (3) query memory topics where `topic LIKE 'projects-%'` and content ILIKE `%${code}%`, extract first 500 chars as preview/description; return merged `EnrichedProject[]` [owner:api-engineer]
- [x] [1.4] [P-2] Create `apps/dashboard/lib/entity-resolution/index.ts` -- barrel export of `parsePeopleMemory`, `resolveContacts`, `enrichProjects`, and all types (`PersonProfile`, `EnrichedProject`) [owner:api-engineer]

## API Batch: Sender Resolution Endpoint

- [x] [2.1] [P-1] Create `apps/dashboard/app/api/resolve/senders/route.ts` -- `GET /api/resolve/senders`; load all contacts via `db.select().from(contacts)`, load `people` memory topic via `db.select().from(memory).where(eq(memory.topic, 'people')).limit(1)`, parse people profiles via `parsePeopleMemory()`, build resolution map via `resolveContacts()`, count sources (contacts_table matches, memory_people matches, compute unresolved by querying `SELECT DISTINCT sender, channel FROM messages WHERE sender IS NOT NULL` and checking which are not in the resolution map); return `SenderResolutionResponse` JSON [owner:api-engineer]

## API Batch: Contact Relations Endpoint

- [x] [3.1] [P-1] Create `apps/dashboard/app/api/contacts/[id]/related/route.ts` -- `GET /api/contacts/:id/related`; load contact by id via `db.select().from(contacts).where(eq(contacts.id, params.id)).limit(1)`, return 404 if not found; extract channel/sender pairs from contact's `channelIds` jsonb [owner:api-engineer]
- [x] [3.2] [P-1] In the same route handler, query messages: for each `(channel, senderId)` pair from channelIds, query `db.select().from(messages).where(and(eq(messages.channel, channel), eq(messages.sender, senderId))).orderBy(desc(messages.createdAt)).limit(50)`; merge results across channels and re-sort by createdAt desc; also compute total `message_count` via a COUNT query across all channel/sender pairs [owner:api-engineer]
- [x] [3.3] [P-2] In the same route handler, query obligations: find obligations where `sourceChannel` matches any of the contact's channels AND a message from this contact exists within +/- 1 hour of the obligation's `createdAt` on the same channel; use a subquery or application-level filter [owner:api-engineer]
- [x] [3.4] [P-2] In the same route handler, query memory profile: load `people` memory topic, parse with `parsePeopleMemory()`, find the profile whose name matches the contact's name (case-insensitive), return the profile's `notes` field as `memory_profile`; return null if no match [owner:api-engineer]
- [x] [3.5] [P-1] Assemble and return `ContactRelatedResponse` JSON with contact, messages, message_count, obligations, memory_profile, channels_active (distinct channels from the contact's channelIds) [owner:api-engineer]

## API Batch: Project Relations Endpoint

- [x] [4.1] [P-1] Create `apps/dashboard/app/api/projects/[code]/related/route.ts` -- `GET /api/projects/:code/related`; read project code from `params.code`; query obligations via `db.select().from(obligations).where(eq(obligations.projectCode, code)).orderBy(desc(obligations.updatedAt))`; compute obligation_summary: total count, count where status = 'open', count where status = 'in_progress', count where status = 'done' [owner:api-engineer]
- [x] [4.2] [P-1] In the same route handler, query sessions: `db.select().from(sessions).where(eq(sessions.project, code)).orderBy(desc(sessions.startedAt))`; compute session_count [owner:api-engineer]
- [x] [4.3] [P-2] In the same route handler, query memory topics: `db.select().from(memory).where(like(memory.topic, 'projects-%'))`, then filter in application code for topics whose content includes the project code (case-insensitive); for each matching topic, return `{ topic: row.topic, preview: row.content.slice(0, 500) }` [owner:api-engineer]
- [x] [4.4] [P-2] In the same route handler, query recent messages mentioning the project: `db.select().from(messages).where(ilike(messages.content, '%' + code + '%')).orderBy(desc(messages.createdAt)).limit(20)`; map to StoredMessage shape [owner:api-engineer]
- [x] [4.5] [P-1] Assemble and return `ProjectRelatedResponse` JSON with project, obligations, obligation_summary, sessions, session_count, memory_topics, recent_messages [owner:api-engineer]

## API Batch: Obligation Relations Endpoint

- [x] [5.1] [P-1] Create `apps/dashboard/app/api/obligations/[id]/related/route.ts` -- `GET /api/obligations/:id/related`; load obligation by id via `db.select().from(obligations).where(eq(obligations.id, params.id)).limit(1)`, return 404 if not found [owner:api-engineer]
- [x] [5.2] [P-2] In the same route handler, find source message: if obligation has `sourceMessage` and `sourceChannel`, query `db.select().from(messages).where(and(eq(messages.channel, obligation.sourceChannel), like(messages.content, '%' + obligation.sourceMessage.slice(0, 100) + '%'), gte(messages.createdAt, new Date(obligation.createdAt.getTime() - 3600000)), lte(messages.createdAt, new Date(obligation.createdAt.getTime() + 3600000)))).limit(1)`; return first match or null [owner:api-engineer]
- [x] [5.3] [P-2] In the same route handler, if obligation has `projectCode`: query obligation count and session count for that project code (reuse logic from project enrichment); return as `{ code, obligation_count, session_count }` or null [owner:api-engineer]
- [x] [5.4] [P-2] In the same route handler, query reminders: `db.select().from(reminders).where(eq(reminders.obligationId, obligation.id))`; map to `{ id, message, due_at, status }` where status is computed from `deliveredAt` and `cancelled` fields [owner:api-engineer]
- [x] [5.5] [P-2] In the same route handler, query related obligations: if `projectCode` is set, `db.select().from(obligations).where(and(eq(obligations.projectCode, obligation.projectCode), ne(obligations.id, obligation.id))).orderBy(desc(obligations.createdAt)).limit(10)`; map to DaemonObligation shape [owner:api-engineer]
- [x] [5.6] [P-1] Assemble and return `ObligationRelatedResponse` JSON with obligation, source_message, project, reminders, related_obligations [owner:api-engineer]

## API Batch: Enriched Projects Route

- [x] [6.1] [P-1] Update `apps/dashboard/app/api/projects/route.ts` -- after reading base project list from `NV_PROJECTS`, call `enrichProjects(projects, db)` to add obligation counts, session counts, memory context, and last activity timestamp; return `EnrichedProject[]` in the response; maintain backward compatibility by keeping `code` and `path` fields at top level [owner:api-engineer]

## API Batch: TypeScript Types

- [x] [7.1] [P-1] Add `PersonProfile` interface to `apps/dashboard/types/api.ts`: name (string), channel_ids (Record<string, string>), role (string | null), notes (string) [owner:api-engineer]
- [x] [7.2] [P-1] Add `SenderResolutionResponse` interface to `apps/dashboard/types/api.ts`: resolutions (Record<string, string>), source_counts ({ contacts_table: number, memory_people: number, unresolved: number }) [owner:api-engineer]
- [x] [7.3] [P-1] Add `EnrichedProject` interface to `apps/dashboard/types/api.ts`: extends ApiProject with description (string | null), memory_context (string | null), obligation_count (number), active_obligation_count (number), session_count (number), last_activity (string | null) [owner:api-engineer]
- [x] [7.4] [P-1] Add `ContactRelatedResponse` interface to `apps/dashboard/types/api.ts`: contact (Contact), messages (StoredMessage[]), message_count (number), obligations (DaemonObligation[]), memory_profile (string | null), channels_active (string[]) [owner:api-engineer]
- [x] [7.5] [P-1] Add `ProjectRelatedResponse` interface to `apps/dashboard/types/api.ts`: project (ApiProject), obligations (DaemonObligation[]), obligation_summary ({ total, open, in_progress, done: number }), sessions (NexusSessionRaw[]), session_count (number), memory_topics (Array<{ topic: string, preview: string }>), recent_messages (StoredMessage[]) [owner:api-engineer]
- [x] [7.6] [P-1] Add `ObligationRelatedResponse` interface to `apps/dashboard/types/api.ts`: obligation (DaemonObligation), source_message (StoredMessage | null), project ({ code: string, obligation_count: number, session_count: number } | null), reminders (Array<{ id: string, message: string, due_at: string, status: string }>), related_obligations (DaemonObligation[]) [owner:api-engineer]

## E2E Batch: Verification

- [ ] [8.1] TypeScript compilation: `npx tsc --noEmit` in `apps/dashboard/` passes with no errors [owner:api-engineer]
- [ ] [8.2] [user] Manual smoke: call `GET /api/resolve/senders` and verify resolution map contains entries from both contacts table and memory people topic
- [ ] [8.3] [user] Manual smoke: call `GET /api/contacts/:id/related` for a known contact and verify messages are returned from the correct channels with correct sender resolution
- [ ] [8.4] [user] Manual smoke: call `GET /api/projects/nv/related` and verify obligations, sessions, and memory topics are returned for the nv project
- [ ] [8.5] [user] Manual smoke: call `GET /api/obligations/:id/related` for an obligation with a known sourceMessage and verify the source message is found
- [ ] [8.6] [user] Manual smoke: call `GET /api/projects` and verify the response includes enrichment fields (obligation_count, session_count, last_activity) alongside the existing code and path fields
- [ ] [8.7] [user] Manual smoke: verify that a Telegram contact previously showing as "7380462766" is now resolved to a name via the sender resolution map
