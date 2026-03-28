# Tool Timing Display

## MODIFIED Requirements

### Requirement: Completed tool durations replace live timer

The `TelegramStreamWriter` MUST track completed tools with their actual durations and display them in the status line. Active tools SHALL show the humanized name without a timer. Completed tools SHALL show the humanized name with actual duration.

#### Scenario: Single tool completes quickly
Given the agent calls Read which completes in 200ms
When the next flush renders the status line
Then it shows "Reading files... (0.2s)"

#### Scenario: Multiple tools complete in sequence
Given the agent calls Glob (1.2s) then Read (0.3s)
When the status line renders after both complete
Then it shows "Searching files 1.2s | Reading files 0.3s"

#### Scenario: Tool currently active while others completed
Given Glob completed (1.2s) and Calendar is still running
When the status line renders
Then it shows "Searching files 1.2s | Checking Calendar..."

#### Scenario: More than 3 completed tools
Given 5 tools have completed
When the status line renders
Then it shows only the last 3 completed tools to keep the display compact

### Requirement: Running total elapsed time

The `TelegramStreamWriter` MUST track the timestamp of the first event and display total elapsed time on each flush. The total SHALL appear on a separate line below the tool status.

#### Scenario: Total elapsed time display
Given the first event arrived 4.2 seconds ago
When the status line renders
Then it includes a line showing the total elapsed time

#### Scenario: Total updates on each flush
Given the first event arrived at t=0
When a flush fires at t=3.5s and another at t=7.0s
Then the first flush shows approximately 3s total and the second shows approximately 7s total
