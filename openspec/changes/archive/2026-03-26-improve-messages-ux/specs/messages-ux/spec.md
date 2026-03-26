# Capability: Messages UX Improvements

## MODIFIED Requirements

### Requirement: Channel color coding uses deterministic hash palette
The `channelAccentColor` function SHALL return a consistent Tailwind color class for any given channel name by hashing the name into a curated palette of 8 colors. Known channels (telegram, discord, slack, cli, api) MUST return their existing brand colors from the current `CHANNEL_COLOR` map. Unknown channels MUST deterministically map to one of the 8 palette colors via a simple string hash. The palette colors MUST have sufficient contrast against both `ds-gray-100` and `ds-gray-200` backgrounds.

#### Scenario: Known channel returns brand color

Given a channel name "telegram"
When `channelAccentColor("telegram")` is called
Then it returns the existing brand color class `text-[#229ED9]`

#### Scenario: Unknown channel returns deterministic palette color

Given a channel name "whatsapp"
When `channelAccentColor("whatsapp")` is called multiple times
Then it returns the same palette color class every time

#### Scenario: Different unknown channels may get different colors

Given channel names "whatsapp" and "signal"
When `channelAccentColor` is called for each
Then the returned colors are determined by their name hash and may differ

## ADDED Requirements

### Requirement: Messages are grouped by hour with time dividers
Messages within a page SHALL be grouped by hour bucket derived from each message's `timestamp` field. Each group MUST display a divider row above its messages showing a human-readable time label (e.g., "Today, 2:00 PM" or "Mar 25, 10:00 AM"). Groups SHALL be ordered newest-first, matching the existing sort order.

#### Scenario: Messages from different hours show dividers

Given 3 messages: one at 2:15 PM, one at 2:45 PM, one at 1:30 PM today
When the messages page renders
Then two time group dividers appear: "Today, 2:00 PM" above the first two messages and "Today, 1:00 PM" above the third

#### Scenario: Messages all in the same hour show one divider

Given 5 messages all timestamped between 3:00 PM and 3:59 PM
When the messages page renders
Then one time group divider appears above all 5 messages

#### Scenario: Messages from different days show date in divider

Given a message from March 24 at 10:00 AM and a message from March 25 at 10:00 AM
When the messages page renders
Then dividers show "Mar 25, 10:00 AM" and "Mar 24, 10:00 AM" respectively

### Requirement: Long messages expand inline with smooth animation
Messages whose content exceeds 3 lines in the collapsed row preview SHALL be truncated with CSS `line-clamp-3` and display a "Show more" toggle. Clicking "Show more" MUST expand the content inline with a CSS `max-height` transition (200ms duration). The toggle MUST change to "Show less" when expanded. This inline expansion MUST be independent of the existing row expand that shows full metadata.

#### Scenario: Short message shows no toggle

Given a message with content "Hello there"
When the message row renders
Then no "Show more" toggle is displayed
And the full content is visible in the preview

#### Scenario: Long message shows truncated with toggle

Given a message with 10 lines of content
When the message row renders
Then the content is truncated at 3 lines
And a "Show more" toggle is visible below the truncated text

#### Scenario: Expanding a long message

Given a truncated message with "Show more" visible
When the user clicks "Show more"
Then the full message content expands inline with a smooth height transition
And the toggle text changes to "Show less"

#### Scenario: Collapsing an expanded message

Given an expanded message with "Show less" visible
When the user clicks "Show less"
Then the content collapses back to 3 lines with a smooth height transition
And the toggle text changes to "Show more"

### Requirement: Channel filter pills show accent color when active
Channel filter buttons SHALL be styled as pills. When a channel pill is active (selected), it MUST display the channel's accent color as a left border or background tint. Inactive pills MUST use the existing neutral style. The "All channels" pill MUST always use neutral styling regardless of selection state.

#### Scenario: Active channel pill shows accent color

Given the user clicks the "telegram" filter pill
When the pill becomes active
Then it displays telegram's brand color (#229ED9) as a visual accent

#### Scenario: Inactive pills remain neutral

Given "telegram" is the active filter
When viewing the "discord" pill
Then it displays in the existing neutral gray style

### Requirement: Message rows show channel accent as left border
Each message row SHALL display a 3px left border using the channel's accent color from `channelAccentColor`. The channel badge within the row SHALL have a subtle background tint derived from the same accent color.

#### Scenario: Telegram message shows blue left border

Given a message from the telegram channel
When the message row renders
Then a 3px left border in #229ED9 is visible on the left edge of the row

#### Scenario: Unknown channel message shows palette-derived border

Given a message from a channel called "matrix"
When the message row renders
Then a 3px left border in the deterministically assigned palette color is visible
