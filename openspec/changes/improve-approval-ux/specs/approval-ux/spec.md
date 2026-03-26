# Capability: Approval UX Enhancements

## ADDED Requirements

### Requirement: Keyboard shortcuts for approval processing
The Approvals page SHALL register keyboard event listeners that map A to approve, D to dismiss, J/K and ArrowUp/ArrowDown to navigate the queue, and Enter to select the focused item. Listeners MUST be scoped to the Approvals page and MUST be cleaned up on unmount. Shortcut hints SHALL appear on hover over each queue item.

#### Scenario: Approve via keyboard

Given the Approvals page is focused and a queue item is highlighted
When the user presses the A key
Then the highlighted item is approved
And the highlight moves to the next item in the queue

#### Scenario: Navigate queue with J/K

Given the Approvals page is focused with 5 queue items and the first item highlighted
When the user presses J three times
Then the fourth item is highlighted
When the user presses K once
Then the third item is highlighted

#### Scenario: Dismiss via keyboard

Given the Approvals page is focused and a queue item is highlighted
When the user presses the D key
Then the highlighted item is dismissed
And the highlight moves to the next item in the queue

#### Scenario: Shortcuts inactive when page not focused

Given the user is focused on a different page or an input field within the Approvals page
When the user presses A or D
Then no approval or dismissal action is triggered

### Requirement: Urgency color coding on queue items
Each queue item SHALL display a colored border based on its urgency level. Items with urgent priority MUST have a red border. Items with medium priority MUST have an amber border. Items with low or unset priority SHALL use the default border styling.

#### Scenario: Urgent item displays red border

Given a queue item with urgency level "urgent"
When the Approvals page renders
Then the item displays with a red border

#### Scenario: Medium item displays amber border

Given a queue item with urgency level "medium"
When the Approvals page renders
Then the item displays with an amber border

#### Scenario: Low or missing urgency uses default border

Given a queue item with urgency level "low" or no urgency field
When the Approvals page renders
Then the item displays with the default border styling

### Requirement: Batch approve and dismiss actions
Queue items SHALL have a checkbox for multi-selection. When one or more items are selected, a floating action bar MUST appear with "Approve All Selected" and "Dismiss All Selected" buttons and a count of selected items. Executing a batch action MUST process all selected items and clear the selection.

#### Scenario: Select multiple items and batch approve

Given the Approvals queue contains 5 items
When the user checks the checkbox on items 1, 3, and 5
Then the floating action bar appears showing "3 selected"
When the user clicks "Approve All Selected"
Then items 1, 3, and 5 are approved
And the floating action bar disappears
And the selection is cleared

#### Scenario: Batch dismiss selected items

Given the user has selected 2 queue items
When the user clicks "Dismiss All Selected"
Then both items are dismissed
And the floating action bar disappears

#### Scenario: No items selected hides action bar

Given no queue items are checked
Then the floating action bar is not visible

### Requirement: Queue clear celebration animation
When the last item in the approval queue is processed and the queue reaches zero items, an "All clear" success illustration SHALL fade in over 800ms. The animation MUST auto-dismiss after display.

#### Scenario: Last item approved triggers celebration

Given the Approvals queue contains exactly 1 item
When the user approves that item
Then the queue is empty
And an "All clear" illustration fades in over 800ms

#### Scenario: Celebration does not appear when items remain

Given the Approvals queue contains 3 items
When the user approves 1 item
Then 2 items remain in the queue
And no celebration animation is shown
