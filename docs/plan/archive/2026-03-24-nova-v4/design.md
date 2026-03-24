# Design System -- Nova v4

## Mark

The Nova mark is an organic constellation with a radar sweep. It represents an intelligent
system that watches, connects, and detects across a natural network of services and channels.

**Variant:** OR6 v6 "Nova (Final)"
**Source:** `wireframes/mark-or6-final.html` (v6 cell)
**File:** `brand/nova-mark.svg`

**Elements:**
- Off-center bright node (Nova core) with radial glow
- 2 bright nearby stars (primary connections)
- 3 medium stars (secondary awareness)
- 3 distant dust particles (background awareness)
- Curved bezier connections (organic, not geometric)
- Faint partial arc (whisper of radar range)
- Gradient sweep line (active scanning)
- Rose alert dot with halo at sweep tip (detection)

**Size behavior:**
- 16px: Core + bright stars + alert dot only
- 24px: Add sweep line
- 32px: Add medium stars + primary curves
- 48px+: Full detail with dust, glow, faint connections
- 112px+: Hero size with all subtlety visible

## Palette

Cosmic Purple + Rose on Vercel Black.

### Core

| Token | Value | Usage |
|-------|-------|-------|
| `--bg` | `#000000` | Page background (Vercel black) |
| `--surface` | `#0a0a0a` | Cards, panels, elevated surfaces |
| `--surface-hover` | `#111111` | Hover states |
| `--border` | `#1a1a1a` | Borders, dividers |
| `--border-hover` | `#2a2a2a` | Interactive border hover |

### Brand

| Token | Value | Usage |
|-------|-------|-------|
| `--primary` | `#7c3aed` | Primary brand (violet-600). Structural accents, active nav, mark lines |
| `--primary-light` | `#a78bfa` | Lighter brand for smaller text on dark bg |
| `--nova` | `#c4b5fd` | Nova identity (violet-300). Badges, marks, note headers |
| `--nova-bg` | `#120a20` | Nova badge/note background |
| `--nova-border` | `#1a1030` | Nova note border |
| `--leo` | `#fb7185` | Leo identity (rose-400). Badges, note headers |
| `--leo-bg` | `#200a14` | Leo badge/note background |
| `--leo-border` | `#2a1020` | Leo note border |
| `--rose` | `#e11d48` | Rose-600. Urgency, danger, alert dots |

### Semantic

| Token | Value | Usage |
|-------|-------|-------|
| `--success` | `#34d399` | Healthy, passing, connected |
| `--warning` | `#fbbf24` | Degraded, stale, attention |
| `--danger` | `#f87171` | Error, failure, overdue |
| `--info` | `#60a5fa` | Informational, neutral accent |

### Text

| Token | Value | Usage |
|-------|-------|-------|
| `--text` | `#ededed` | Primary text (Geist standard) |
| `--text-muted` | `#888888` | Secondary text, labels |
| `--text-dim` | `#444444` | Tertiary, timestamps, hints |

## Typography

Geist type system (Vercel). Sans for UI, Mono for data.

| Token | Family | Weight | Size | Usage |
|-------|--------|--------|------|-------|
| `--font-sans` | Geist Sans | 400/600 | -- | Nav, headings, body text, labels |
| `--font-mono` | Geist Mono | 400/500 | -- | Data values, badges, code, tool names, session IDs |

### Scale

| Token | Size | Usage |
|-------|------|-------|
| `--text-xs` | 9px | Swatch labels, sublabels |
| `--text-sm` | 11px | Badges, metadata, timestamps |
| `--text-base` | 13px | Body text, table cells |
| `--text-lg` | 16px | Section headers |
| `--text-xl` | 20px | Session timers |
| `--text-2xl` | 28px | Stat card values |

## Spacing

4px base unit.

| Token | Value |
|-------|-------|
| `--space-1` | 4px |
| `--space-2` | 8px |
| `--space-3` | 12px |
| `--space-4` | 16px |
| `--space-6` | 24px |
| `--space-8` | 32px |

## Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 4px | Badges, small elements |
| `--radius-md` | 6px | Notes, inputs |
| `--radius-lg` | 8px | Cards, sessions |
| `--radius-xl` | 12px | Panels, modals |

## Component Patterns

### Badges

```css
.badge-nova { background: var(--nova-bg); color: var(--nova); border: 1px solid var(--nova-border); }
.badge-leo  { background: var(--leo-bg);  color: var(--leo);  border: 1px solid var(--leo-border); }
.badge-p0   { background: #200a0a; color: var(--danger); }
.badge-p1   { background: #1a1a0a; color: var(--warning); }
.badge-p2   { background: #0a0a1a; color: var(--info); }
```

### Nova Notes

```css
.nova-note { background: var(--nova-bg); border: 1px solid var(--nova-border); color: var(--nova); }
.leo-note  { background: var(--leo-bg);  border: 1px solid var(--leo-border);  color: var(--leo); }
```

### Status Dots

```css
.dot-ok   { background: var(--success); }
.dot-warn { background: var(--warning); }
.dot-err  { background: var(--danger); }
.dot-off  { background: var(--text-dim); }
```

### Obligation Priority Bars

3px left border on obligation items, color matches priority:
- P0: `var(--danger)` (red)
- P1: `var(--warning)` (amber)
- P2: `var(--info)` (blue)
- P3: `var(--text-dim)` (gray)

## Layout

- Fixed sidebar (220px), main body scrolls independently
- Sidebar: `var(--surface)` background, `var(--border)` right edge
- Sidebar brand includes Nova mark (20px), name, status, usage sparkline
- Content max-width: none (fills available space, data-dense)

## Icon Sources

| Source | Usage | URL Pattern |
|--------|-------|-------------|
| svgl.app | Brand/service logos | `https://svgl.app/library/{name}.svg` |
| Simple Icons | Brands not in svgl (Jira) | `https://cdn.simpleicons.org/{name}/{color}` |
| Phosphor Icons | UI icons (pending) | React component library |
| Nova Mark SVG | Nova identity | `brand/nova-mark.svg` |

## Anti-Patterns

- No emoji anywhere. Ever.
- No Lucide, Heroicons, or Font Awesome (Phosphor is the UI icon library)
- No full-circle radar rings (use partial arcs for organic feel)
- No forced symmetry in constellation elements
- No bright brand colors on `--bg` directly (always on `--surface` or darker)
- No `#fff` white text (use `--text` which is `#ededed`)
