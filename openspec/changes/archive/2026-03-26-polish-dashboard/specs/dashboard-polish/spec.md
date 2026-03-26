# Capability: Dashboard Polish

## MODIFIED Requirements

### Requirement: Greeting banner replaces static header
The dashboard home page SHALL replace the static "Dashboard" / "Nova activity overview" header with a personalized greeting banner. The banner MUST display "Good morning/afternoon/evening, Leo" based on the client's local time (morning 05:00--11:59, afternoon 12:00--16:59, evening 17:00--04:59) and today's date. The banner SHALL fire-and-forget fetch `/api/briefing` and append a one-line summary when the response arrives. The greeting MUST render immediately without waiting for the briefing API. If the API call fails or exceeds 3 seconds, the greeting MUST display without a summary.

#### Scenario: Morning greeting with briefing summary

Given the client local time is 08:30
And the `/api/briefing` endpoint returns `{ summary: "3 obligations due today" }` within 3 seconds
When the dashboard home page loads
Then the header displays "Good morning, Leo" with today's date
And the briefing summary "3 obligations due today" appears below the greeting

#### Scenario: Evening greeting without briefing (API failure)

Given the client local time is 19:00
And the `/api/briefing` endpoint returns a 500 error
When the dashboard home page loads
Then the header displays "Good evening, Leo" with today's date
And no briefing summary line is shown
And no error is surfaced to the user

#### Scenario: Briefing API slow (>3s timeout)

Given the client local time is 14:00
And the `/api/briefing` endpoint takes 5 seconds to respond
When the dashboard home page loads
Then the header displays "Good afternoon, Leo" with today's date within 100ms of page load
And the briefing summary is not shown (request abandoned or ignored after 3s)

### Requirement: Last-updated timestamp on auto-refresh
The dashboard MUST display an "Updated Xs ago" label next to the auto-refresh toggle. The label SHALL update every 1 second via a client-side interval. On hover, the label MUST show the exact ISO timestamp of the last successful fetch via a title attribute. The label SHALL reset to "Updated just now" on each successful data fetch. When auto-refresh is disabled, the timestamp MUST continue ticking up from the last fetch time.

#### Scenario: Timestamp after fresh fetch

Given auto-refresh is enabled
When a successful data fetch completes
Then the timestamp label resets to "Updated just now"
And the label increments to "Updated 1s ago", "Updated 2s ago", etc. each second

#### Scenario: Hover shows exact time

Given the last fetch completed at 2026-03-26T14:30:45.000Z
When the user hovers over the timestamp label
Then the title attribute shows "2026-03-26T14:30:45.000Z"

#### Scenario: Auto-refresh disabled

Given auto-refresh is toggled off
And the last fetch was 45 seconds ago
When 10 more seconds pass
Then the label shows "Updated 55s ago"

### Requirement: Stat cards grouped into operational and performance rows
The 6 existing stat cards MUST be split into two visually distinct rows. The top row SHALL be labeled "Operational" and contain Obligations, Active Sessions, and Health cards. The bottom row SHALL be labeled "Performance" and contain Cold Starts, Five-Byte, and Tokens cards. Each group MUST have a muted text label above it. Card order within each group MUST match the current left-to-right display order.

#### Scenario: Two-row layout renders

Given the dashboard home page loads with all 6 stat cards
When the page renders
Then the top row displays 3 cards (Obligations, Active Sessions, Health) under a muted "Operational" label
And the bottom row displays 3 cards (Cold Starts, Five-Byte, Tokens) under a muted "Performance" label

### Requirement: Disconnected state overlay on stat cards
When the daemon WebSocket connection is disconnected, the dashboard MUST dim all stat cards using opacity reduction. Each card MUST show an "Offline" badge instead of displaying "0" or stale numeric values. The auto-refresh toggle MUST be greyed out or hidden while disconnected. When the connection restores, the overlay MUST be removed and normal display MUST resume immediately.

#### Scenario: Daemon disconnects

Given the dashboard is displaying live stat values
When the daemon WebSocket connection drops
Then all 6 stat cards are dimmed (reduced opacity)
And each card shows an "Offline" badge
And the auto-refresh toggle is greyed out

#### Scenario: Daemon reconnects

Given the dashboard is in disconnected state with dimmed cards
When the daemon WebSocket connection restores
Then card opacity returns to normal
And the "Offline" badges are removed
And stat values update to current data
And the auto-refresh toggle becomes interactive again
