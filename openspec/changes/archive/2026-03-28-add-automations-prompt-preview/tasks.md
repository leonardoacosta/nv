# Implementation Tasks

<!-- beads:epic:nv-cpc5 -->

## DB Batch

(No schema changes -- obligations, reminders, memory, messages tables already exist in @nova/db)

## API Batch

- [x] [2.1] [P-1] Add `automation.previewContext` tRPC procedure in `packages/api/src/routers/automation.ts`. Input: `{ type: "watcher" | "briefing" }`. Query obligations (status IN open, in_progress, pending; limit 20), memory (latest 10 by updated_at), messages (latest 50, all channels), and obligation stats (counts by status). Use `Promise.allSettled` with 5s timeout per query. Return `{ obligations: { items, countByStatus }, memory: { items: [{ topic, contentPreview }] }, messages: { byChannel: [{ channel, count, latestPreview }] }, channels: [{ name, messageCount, active }], stats: { totalObligations, activeReminders, memoryTopics }, assembledAt: ISO string }`. Include source status per section (ok/unavailable/empty). [owner:api-engineer] [beads:nv-1rzz]

## UI Batch

- [x] [3.1] [P-1] Add TypeScript types to `apps/dashboard/types/api.ts`: `AutomationContextPreview` (top-level response), `ContextObligationSummary` (items + countByStatus), `ContextMemoryItem` (topic + contentPreview), `ContextChannelSummary` (channel + count + latestPreview), `ChannelInfo` (name + messageCount + active), `ContextStats` (totalObligations + activeReminders + memoryTopics). [owner:ui-engineer] [beads:nv-d6na]
- [x] [3.2] [P-1] Create `PromptPreviewDrawer` component in `apps/dashboard/app/automations/page.tsx`. Slide-in from right (560px desktop, full-width mobile) with backdrop overlay. Props: `open: boolean`, `onClose`, `automationType: "watcher" | "briefing"`, `customPrompt: string`. Fetches `automation.previewContext` on open. Renders sections: static system prompt preamble (read-only code block), custom prompt (highlighted), context sections (obligations, memory, messages by channel, stats). Shows "assembled at" timestamp. Lazy-loaded via `React.lazy`. [owner:ui-engineer] [beads:nv-94i7]
- [x] [3.3] [P-2] Add channel source pills inside `PromptPreviewDrawer`. Render a horizontal row of pills for each channel (Telegram, Discord, Teams, Email, Dashboard). Active channels show message count badge and highlighted border; inactive channels are dimmed with 0 count. Derive from `channels` array in the preview response. [owner:ui-engineer] [beads:nv-xeqx]
- [x] [3.4] [P-2] Add filter controls to `PromptPreviewDrawer` header. Time range dropdown (1h, 6h, 12h, 24h, 7d; default 24h). Obligation status multi-select chips (open, in_progress, proposed_done; default open + in_progress). Channel checkboxes to include/exclude. All filters applied client-side to returned context data with 300ms debounce. Filter state is local (not persisted). [owner:ui-engineer] [beads:nv-i7iy]
- [x] [3.5] [P-1] Add "Preview Prompt" button to `WatcherCard` and `BriefingCard` components in `apps/dashboard/app/automations/page.tsx`. Position next to the "Custom Prompt" collapsible trigger. Button opens the `PromptPreviewDrawer` with the appropriate `automationType`. Wire drawer open/close state in the parent `AutomationsPage` component. [owner:ui-engineer] [beads:nv-5fk0]
- [x] [3.6] [P-1] Add context summary bar above each automation card in the grid layout. Shows: active obligations count (N open, M in progress), memory topics loaded (count), messages in context (count by channel), last assembly timestamp. Data sourced from the `automation.previewContext` response, cached and refreshed on the 30s polling cycle. Compact single-line layout using `text-copy-13` tokens. [owner:ui-engineer] [beads:nv-9iem]
- [x] [3.7] [P-2] Add reminders-vs-obligations info card in the "Scheduled Automations" section, between `SectionHeader` and the tab control. Collapsible (default collapsed) with Info icon trigger. Explains: obligations = detected commitments with lifecycle (open -> in_progress -> proposed_done -> done); reminders = one-shot alerts, optionally linked to obligations via FK. First-visit pulsing dot via localStorage flag. Uses `ds-gray-alpha-100` surface. [owner:ui-engineer] [beads:nv-dwrv]
- [x] [3.8] [P-2] Update "Reminders" tab label to "Reminders (Alerts)" in the segmented control to clarify the distinction from obligations. [owner:ui-engineer] [beads:nv-mqyk]
- [x] [3.9] [P-2] Add drawer slide-in animation keyframes to `apps/dashboard/app/globals.css` if not already present. Use `translateX(100%)` -> `translateX(0)` with 200ms ease-out transition. Include backdrop fade-in. [owner:ui-engineer] [beads:nv-r7g4]

## E2E Batch

- [x] [4.1] Verify dashboard builds cleanly: `pnpm typecheck` passes with new types, drawer component, preview button wiring, info card, and context summary bar. [owner:e2e-engineer] [beads:nv-7xo6]
