# Implementation Tasks

<!-- beads:epic:nv-j35x -->

## DB Batch

- [x] [1.1] [P-1] Add thread_id (text, nullable, indexed) and reply_to_message_id (integer, nullable) columns to messages schema [owner:db-engineer] [beads:nv-gqh9]
- [x] [1.2] [P-2] Generate and apply migration via drizzle-kit [owner:db-engineer] [beads:nv-fqme]

## API Batch

- [x] [2.1] [P-1] Add threadId and replyToMessageId fields to Message interface in types.ts [owner:api-engineer] [beads:nv-klnw]
- [x] [2.2] [P-1] Create ThreadResolver class with in-memory cache and DB chain-walk fallback [owner:api-engineer] [beads:nv-865y]
- [x] [2.3] [P-1] Capture reply_to_message.message_id in normalizeTextMessage, normalizeVoiceMessage, normalizePhotoMessage [owner:api-engineer] [beads:nv-km78]
- [x] [2.4] [P-1] Refactor JobQueue from flat FIFO to per-thread sub-queues with global concurrency cap [owner:api-engineer] [beads:nv-dn0p]
- [x] [2.5] [P-1] Add threadId to Job and EnqueueOpts types in queue/types.ts [owner:api-engineer] [beads:nv-bk5c]
- [x] [2.6] [P-2] Update ConversationManager.loadHistory to accept optional threadId filter [owner:api-engineer] [beads:nv-gbod]
- [x] [2.7] [P-2] Update ConversationManager.saveExchange to persist thread_id and reply_to_message_id [owner:api-engineer] [beads:nv-xjsk]
- [x] [2.8] [P-2] Add replyToMessageId parameter to TelegramStreamWriter constructor and wire through finalize/sendDraft [owner:api-engineer] [beads:nv-r6od]
- [x] [2.9] [P-2] Update index.ts message routing: resolve thread, pass threadId to enqueue, pass replyToMessageId to StreamWriter [owner:api-engineer] [beads:nv-o9ji]
- [x] [2.10] [P-2] Fix queue ack messages to use thread-aware status and reply_to the user's message [owner:api-engineer] [beads:nv-fysy]
- [x] [2.11] [P-3] Add 60s thread sub-queue cleanup timer [owner:api-engineer] [beads:nv-t86s]
- [x] [2.12] [P-3] Add failed event handler that notifies user via Telegram on job failure [owner:api-engineer] [beads:nv-ekze]

## UI Batch

(No UI tasks — daemon-only change)

## E2E Batch

- [x] [4.1] Test: unquoted message creates new thread, response has reply_to [owner:e2e-engineer] [beads:nv-kwle]
- [x] [4.2] Test: quoted message joins existing thread, serial execution within thread [owner:e2e-engineer] [beads:nv-fip4]
- [x] [4.3] Test: two unquoted messages process in parallel (different threads) [owner:e2e-engineer] [beads:nv-3zyq]
- [x] [4.4] Test: queue position ack is per-thread accurate [owner:e2e-engineer] [beads:nv-qeo4]
