# Implementation Tasks
<!-- beads:epic:nv-cd9i -->

## UI Batch

- [ ] [3.1] [P-1] Add deterministic channel color hashing utility — create a `channelAccentColor(name: string): string` function that returns a Tailwind color class from a curated 8-color palette, hashed by channel name; known channels (telegram, discord, slack, cli, api) return their existing brand colors [owner:ui-engineer] [beads:nv-py83]
- [ ] [3.2] [P-1] Add time grouping logic — create a `groupMessagesByHour(messages: StoredMessage[]): { label: string; messages: StoredMessage[] }[]` helper that groups messages by hour bucket from `timestamp`; render a subtle divider row with the time label between groups [owner:ui-engineer] [beads:nv-nd6y]
- [ ] [3.3] [P-1] Add inline expand/collapse for long messages — in `MessageRow`, truncate content preview at 3 lines (using `line-clamp-3`); add a "Show more" / "Show less" toggle that expands inline with a CSS `max-height` transition (200ms); independent of the existing metadata expand [owner:ui-engineer] [beads:nv-8ik5]
- [ ] [3.4] [P-2] Apply channel accent color to message rows — add a 3px left border using the channel's accent color to each `MessageRow`; add a subtle background tint to the channel badge [owner:ui-engineer] [beads:nv-981h]
- [ ] [3.5] [P-2] Upgrade channel filter tabs to pill-style — apply the channel's accent color as left border or background tint on active pills; inactive pills remain neutral; "All channels" pill stays neutral [owner:ui-engineer] [beads:nv-l1xp]
