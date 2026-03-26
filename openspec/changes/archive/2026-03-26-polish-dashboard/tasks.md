# Implementation Tasks

<!-- beads:epic:nv-jtuy -->

## UI Batch

- [ ] [1.1] [P-1] Add greeting banner component to `apps/dashboard/app/page.tsx` --- replace the "Dashboard" / "Nova activity overview" header with a time-of-day greeting ("Good morning/afternoon/evening, Leo") and today's date; fire-and-forget fetch to `/api/briefing` to append a one-line summary when available; never block render on the API call [owner:ui-engineer] [beads:nv-bzim]
- [ ] [1.2] [P-1] Add last-updated timestamp next to the auto-refresh toggle in `apps/dashboard/app/page.tsx` --- display "Updated Xs ago" with a 1s interval tick; show exact ISO timestamp on hover via title attribute; reset to "Updated just now" on each successful data fetch [owner:ui-engineer] [beads:nv-dwof]
- [ ] [1.3] [P-1] Split 6 stat cards into 2 grouped rows in `apps/dashboard/app/page.tsx` --- top row "Operational" (Obligations, Active Sessions, Health), bottom row "Performance" (Cold Starts, Five-Byte, Tokens); add muted group labels above each row [owner:ui-engineer] [beads:nv-fq2e]
- [ ] [1.4] [P-1] Add disconnected state overlay to stat cards in `apps/dashboard/app/page.tsx` --- when daemon WebSocket is disconnected, dim cards with opacity reduction, show "Offline" badge on each card, grey out auto-refresh toggle; restore normal display on reconnect [owner:ui-engineer] [beads:nv-xemt]
- [ ] [1.5] [P-2] Run `pnpm typecheck` in `apps/dashboard/` --- zero TypeScript errors [owner:ui-engineer] [beads:nv-wgzz]
