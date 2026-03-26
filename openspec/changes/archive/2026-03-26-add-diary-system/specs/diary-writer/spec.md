# Capability: Diary Writer

## ADDED Requirements

### Requirement: writeEntry inserts diary row after Agent SDK response
`DiaryWriter` SHALL expose a `writeEntry(input: DiaryWriteInput): Promise<void>` function that inserts one row into the `diary` Postgres table via Drizzle after each Agent SDK response cycle. All insert errors MUST be caught and logged at warn level; the function MUST never re-throw.

#### Scenario: Successful insert after message processed

Given the Agent SDK has returned a response for a Telegram message
When `writeEntry({ trigger_type: "telegram_message", channel: "telegram", slug: "...", tools_used: ["bash"], tokens_in: 120, tokens_out: 340, response_latency_ms: 1850, content: "Created Jira ticket XY-123" })` is called
Then a row is inserted into the `diary` table with all provided fields and `created_at` set to the current UTC timestamp

#### Scenario: Insert failure is swallowed

Given a Postgres connection error
When `writeEntry(...)` is called
Then the error is caught and logged at warn level
And the function resolves without throwing
And the calling agent handler continues normally

#### Scenario: tools_used serialized as JSONB

Given `tools_used: ["bash", "jira_create"]`
When the row is inserted
Then the `tools_used` column stores a JSON array `["bash", "jira_create"]`

#### Scenario: Empty tools array

Given `tools_used: []`
When the row is inserted
Then `tools_used` stores `[]` (not null)
