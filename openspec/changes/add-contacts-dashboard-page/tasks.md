# Implementation Tasks

<!-- beads:epic:nv-m1tq -->

## API Batch

- [ ] [1.1] [P-1] Create `apps/dashboard/app/api/contacts/route.ts` — GET handler: proxy `?q=` and `?relationship=` params to daemon `/api/contacts`; return daemon response as-is; 502 on unreachable [owner:api-engineer]
- [ ] [1.2] [P-1] Add POST handler to `apps/dashboard/app/api/contacts/route.ts` — forward JSON body to daemon `POST /api/contacts`; return daemon response as-is; 502 on unreachable [owner:api-engineer]
- [ ] [1.3] [P-1] Create `apps/dashboard/app/api/contacts/[id]/route.ts` — GET, PATCH (mapped to daemon PUT), DELETE handlers; use `params: Promise<{ id: string }>` pattern matching `obligations/[id]/route.ts` [owner:api-engineer]

## Types Batch

- [ ] [2.1] [P-1] Add `Contact` interface to `apps/dashboard/types/api.ts` — fields: id, name, channel_ids (telegram/discord/teams optional string keys), relationship_type union, notes nullable string, created_at/updated_at strings [owner:api-engineer]

## UI Batch

- [ ] [3.1] [P-1] Create `apps/dashboard/app/contacts/page.tsx` — `"use client"` component; fetch `GET /api/contacts` on mount; store contacts array in local state; page header with title "Contacts", count subtitle, Refresh button [owner:ui-engineer]
- [ ] [3.2] [P-1] Add debounced search bar (300ms) with `Search` icon inset left — on change re-fetches `GET /api/contacts?q=<value>`; clear resets to full list [owner:ui-engineer]
- [ ] [3.3] [P-1] Add relationship filter chip strip — `All | Work | Personal | Contributor | Social`; active chip styled `bg-cosmic-purple/20 text-cosmic-bright`; selecting non-All chip adds `?relationship=<type>` to fetch; `personal-client` maps to label "Personal" [owner:ui-engineer]
- [ ] [3.4] [P-1] Render contact cards — each card shows: Name (cosmic-bright), relationship badge (color-coded pill per type), channel identifiers (non-empty channels only, "Channel: value" format), notes preview (80 char truncated), Edit + Delete icon buttons right-aligned [owner:ui-engineer]
- [ ] [3.5] [P-1] Loading state: 5 animate-pulse skeleton cards; error state: rose alert with AlertCircle; empty state: Users icon + "No contacts yet" text + "Create contact" button [owner:ui-engineer]
- [ ] [3.6] [P-1] Implement create/edit modal — inline controlled component, no external library; `modalState: { mode: "closed" } | { mode: "create" } | { mode: "edit"; contact: Contact }`; backdrop `bg-black/60 backdrop-blur-sm` dismisses on click; Escape key closes [owner:ui-engineer]
- [ ] [3.7] [P-1] Modal fields: Name (required text), Relationship (select, default "social"), Telegram identifier (text, optional), Discord identifier (text, optional), Teams UPN (text, optional), Notes (textarea 3 rows, optional) [owner:ui-engineer]
- [ ] [3.8] [P-1] Modal submit: Create mode → POST `/api/contacts`; Edit mode → PATCH `/api/contacts/{id}`; on success close modal + re-fetch; inline rose error text on failure [owner:ui-engineer]
- [ ] [3.9] [P-1] Delete confirmation: clicking Delete on a card enters inline confirm state (button text "Confirm?" + Cancel link, auto-reset after 2s); confirm triggers DELETE `/api/contacts/{id}` + re-fetch on success [owner:ui-engineer]

## Sidebar

- [ ] [4.1] [P-1] Import `Users` from lucide-react in `apps/dashboard/components/Sidebar.tsx` [owner:ui-engineer]
- [ ] [4.2] [P-1] Add `{ to: "/contacts", label: "Contacts", icon: Users }` to `NAV_ITEMS` after the `/messages` entry [owner:ui-engineer]

## Verify

- [ ] [5.1] `pnpm typecheck` passes in `apps/dashboard` [owner:ui-engineer]
- [ ] [5.2] `pnpm build` passes in `apps/dashboard` [owner:ui-engineer]
- [ ] [5.3] [user] Manual: navigate to `/contacts` — page loads (empty state or list if contacts exist) [owner:ui-engineer]
- [ ] [5.4] [user] Manual: create a contact via modal, verify it appears in the list [owner:ui-engineer]
- [ ] [5.5] [user] Manual: edit a contact — changes persist on re-load [owner:ui-engineer]
- [ ] [5.6] [user] Manual: delete a contact — card disappears from list [owner:ui-engineer]
- [ ] [5.7] [user] Manual: search and relationship filter return correct subsets [owner:ui-engineer]
