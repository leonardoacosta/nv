# Implementation Tasks

<!-- beads:epic:nv-4u1 -->

## DB Batch

- [ ] [1.1] [P-1] Populate config/user.md with actual operator details [owner:db-engineer] [beads:nv-flo]
- [ ] [1.2] [P-1] Populate config/identity.md with ✨ emoji and remove placeholders [owner:db-engineer] [beads:nv-4ym]

## API Batch

- [ ] [2.1] [P-1] Create conversation.rs — ConversationStore with session expiry and bounds [owner:api-engineer] [beads:nv-xw9]
- [ ] [2.2] [P-1] Add tool_result truncation (>1000 chars) in ConversationStore::push [owner:api-engineer] [beads:nv-2un]
- [ ] [2.3] [P-2] Register ConversationStore in SharedDeps and construct in main.rs [owner:api-engineer] [beads:nv-am6]
- [ ] [2.4] [P-2] Bump format_recent_for_context truncation 500→2000 + turn-pair grouping [owner:api-engineer] [beads:nv-37e]

## UI Batch

- [ ] [3.1] [P-1] Wire ConversationStore into Worker::run — load prior turns + push completed turns [owner:ui-engineer] [beads:nv-vra]
- [ ] [3.2] [P-2] Move history constants from agent.rs to conversation.rs, remove dead_code attrs [owner:ui-engineer] [beads:nv-56z]

## E2E Batch

- [ ] [4.1] Unit tests: ConversationStore push/load/expire/trim [owner:test-writer] [beads:nv-4eq]
- [ ] [4.2] Unit test: tool_result truncation at 1000 chars [owner:test-writer] [beads:nv-wcd]
- [ ] [4.3] Unit test: format_recent_for_context 2000-char limit + turn grouping [owner:test-writer] [beads:nv-20g]
