# Proposal: Add Contacts Dashboard Page

## Change ID
`add-contacts-dashboard-page`

## Summary

Implement the missing `/contacts` dashboard page and its API proxy routes. The
`contact-profiles-system` spec created the Rust `ContactStore` and all daemon API endpoints but
never wired the Next.js UI. This spec adds the contacts list page, the API proxy routes, the
TypeScript types, and the Sidebar nav entry.

## Context
- Extends: `apps/dashboard/app/`, `apps/dashboard/app/api/`, `apps/dashboard/types/api.ts`,
  `apps/dashboard/components/Sidebar.tsx`
- Depends on: `contact-profiles-system` (daemon endpoints already deployed),
  `fix-dashboard-api-proxy` (proxy pattern must be stable before adding new routes)
- Related: Daemon routes `GET/POST /api/contacts`, `GET/PUT/DELETE /api/contacts/{id}`
  (from `crates/nv-daemon/src/http.rs`)

## Motivation

The `/contacts` route returns 404. The backend is fully functional — the `ContactStore` and all
five HTTP routes (`GET/POST /api/contacts`, `GET/PUT/DELETE /api/contacts/{id}`) shipped with
`contact-profiles-system`. The only missing piece is the frontend. Operators cannot manage
contacts, assign relationship types, or associate channel identifiers without a dashboard page.

This spec closes that gap with a single focused UI sprint: no new Rust code, no schema changes,
just the Next.js page, its API proxy routes, and the Sidebar link.

## Requirements

### Req-1: API Proxy — `GET /api/contacts`

`apps/dashboard/app/api/contacts/route.ts`

Proxy to daemon `GET /api/contacts`. Forward `?q=` (search) and `?relationship=` (filter) query
params to the daemon URL. Return daemon response as-is with matching HTTP status.

Error handling: if daemon is unreachable, return `{ "error": "Daemon unreachable" }` with
status 502 — consistent with all other API proxy routes in this project.

### Req-2: API Proxy — `POST /api/contacts`

Same file (`route.ts`) — add `POST` handler. Forward JSON body to daemon `POST /api/contacts`.
Return daemon response as-is.

### Req-3: API Proxy — `GET/PATCH/DELETE /api/contacts/[id]`

`apps/dashboard/app/api/contacts/[id]/route.ts`

Three handlers in one route file:
- `GET`: proxy to daemon `GET /api/contacts/{id}`
- `PATCH`: proxy JSON body to daemon `PUT /api/contacts/{id}` (dashboard uses PATCH; daemon
  accepts PUT — map accordingly)
- `DELETE`: proxy to daemon `DELETE /api/contacts/{id}`

Follow the same `params: Promise<{ id: string }>` pattern used by
`apps/dashboard/app/api/obligations/[id]/route.ts`.

### Req-4: TypeScript Types — `Contact`

Add to `apps/dashboard/types/api.ts`:

```typescript
// ── GET /api/contacts ──────────────────────────────────────────────────────

export interface Contact {
  id: string;
  name: string;
  channel_ids: {
    telegram?: string;
    discord?: string;
    teams?: string;
    [key: string]: string | undefined;
  };
  relationship_type: "work" | "personal-client" | "contributor" | "social";
  notes: string | null;
  created_at: string;
  updated_at: string;
}
```

Types derived from the daemon `Contact` struct in
`crates/nv-daemon/src/contact_store.rs`. The `GET /api/contacts` handler returns
`Vec<Contact>` serialized as a plain JSON array (not wrapped in an object key) —
confirmed by reading `list_contacts_handler` in `crates/nv-daemon/src/http.rs`.

### Req-5: Contacts Page — List + Search + Filter

`apps/dashboard/app/contacts/page.tsx` — `"use client"` component.

**Page header**: "Contacts" title, muted subtitle showing count, Refresh button (same pattern as
`ObligationsPage`).

**Search bar**: text input with debounce (300ms), `Search` Lucide icon inset left, placeholder
"Search contacts…". On input change, triggers re-fetch with `?q=<value>`.

**Relationship filter chips**: a horizontal strip of pill buttons: `All`, `Work`,
`Personal`, `Contributor`, `Social`. Active chip uses `bg-cosmic-purple/20 text-cosmic-bright`.
Selecting a chip sets `?relationship=<type>` on the fetch (except "All" which omits the param).
`personal-client` maps to label "Personal" for display.

**Contact cards**: a vertically stacked list of cards. Each card shows:
- **Name** — `text-cosmic-bright font-medium`
- **Relationship badge** — small pill with relationship type; color-coded:
  `work` → `bg-cosmic-purple/20 text-cosmic-purple`,
  `personal-client` → `bg-cosmic-rose/20 text-cosmic-rose`,
  `contributor` → `bg-amber-500/20 text-amber-400`,
  `social` → `bg-emerald-500/20 text-emerald-400`
- **Channels** — space-separated channel identifiers prefixed by channel name, e.g.
  "Telegram: @lacosta · Discord: 123456". Only show channels with a non-empty value.
- **Notes preview** — first 80 chars of notes, truncated with `…` if longer. Hidden if null.
- **Edit** and **Delete** action buttons (icon buttons, right-aligned).

**Loading state**: 5 pulse skeleton cards (same pattern as `ObligationsPage`).

**Error state**: rose-tinted alert with `AlertCircle` icon.

**Empty state**: `Users` Lucide icon + "No contacts yet" message + "Create contact" button
that opens the create modal.

### Req-6: Create / Edit Modal

Inline modal — no external modal library. Controlled by a `modalState` local state:
`{ mode: "closed" } | { mode: "create" } | { mode: "edit"; contact: Contact }`.

**Backdrop**: fixed inset overlay `bg-black/60 backdrop-blur-sm`, click to close.

**Panel**: centered card `max-w-md w-full bg-cosmic-dark border border-cosmic-border rounded-cosmic`.

**Fields**:
| Field | Type | Required |
|-------|------|----------|
| Name | text input | yes |
| Relationship | select dropdown | yes (default: "social") |
| Telegram identifier | text input | no (placeholder "@handle") |
| Discord identifier | text input | no (placeholder "user ID") |
| Teams UPN | text input | no (placeholder "user@company.com") |
| Notes | textarea (3 rows) | no |

**Actions**: "Cancel" button (closes modal), "Save" / "Create" button (submits).

On submit:
- **Create**: `POST /api/contacts` with `{ name, channel_ids, relationship_type, notes }`.
  On success: close modal, re-fetch list.
- **Edit**: `PATCH /api/contacts/{id}` with same body. On success: close modal, re-fetch list.

Inline error display if the API call fails (rose text under the form).

### Req-7: Delete Confirmation

Inline confirm state — no separate modal. When the Delete button is clicked on a card, that
card's delete button text changes to "Confirm?" with a Cancel option (2-second auto-reset).
On confirm: `DELETE /api/contacts/{id}`. On success: re-fetch list.

### Req-8: Sidebar Nav Entry

Add to `NAV_ITEMS` in `apps/dashboard/components/Sidebar.tsx`:

```typescript
{ to: "/contacts", label: "Contacts", icon: Users },
```

Import `Users` from `lucide-react` (already a dependency). Insert after the
`/messages` entry (position: between Messages and Projects).

## Scope
- **IN**: API proxy routes (GET, POST, GET/PATCH/DELETE by ID), TypeScript types, contacts list
  page with search + filter + cards, create/edit modal, delete confirmation, Sidebar nav entry.
- **OUT**: Contacts page pagination (list is small, all-at-once is fine), contact avatar/photo
  upload, contact merge UI, bulk import/export, per-contact message history view.

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/contacts/page.tsx` | New: full contacts list page with search, filter chips, contact cards, modal, delete confirm |
| `apps/dashboard/app/api/contacts/route.ts` | New: GET + POST proxy to daemon `/api/contacts` |
| `apps/dashboard/app/api/contacts/[id]/route.ts` | New: GET + PATCH + DELETE proxy to daemon `/api/contacts/{id}` |
| `apps/dashboard/types/api.ts` | Add `Contact` interface |
| `apps/dashboard/components/Sidebar.tsx` | Add `Users` icon import + Contacts nav entry after Messages |

## Risks
| Risk | Mitigation |
|------|-----------|
| Daemon `PUT /api/contacts/{id}` vs dashboard `PATCH` mismatch | Proxy maps PATCH → PUT explicitly; trivial fix if daemon is updated to accept PATCH |
| Daemon response shape changes | Types confirmed from source code at spec-write time; `Array.isArray` guard on list parse path for safety |
| Search debounce causes stale results if user types quickly | Debounce ref cleaned up on unmount; cancel pending fetch on new keypress |
| Modal focus trap not implemented | Non-critical for internal tool; Escape key closes modal as minimum viable accessibility |
