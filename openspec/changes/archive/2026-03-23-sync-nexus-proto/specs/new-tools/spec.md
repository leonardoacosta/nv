# New Tools

## MODIFIED Requirements

### Requirement: start_session MUST support agent targeting

The `start_session` tool SHALL accept an optional `agent` parameter. When provided,
only that specific agent is tried instead of round-robin across all connected agents.

#### Scenario: Start session on specific agent
Given "homelab" and "macbook" agents are connected
When Claude calls `start_session("oo", "/apply fix-bugs", agent="homelab")`
Then only the "homelab" agent receives the StartSession RPC
And "macbook" is not tried

#### Scenario: Start session with no agent (round-robin unchanged)
Given "homelab" and "macbook" agents are connected
When Claude calls `start_session("oo", "/apply fix-bugs")`
Then agents are tried in order until one succeeds (existing behavior)

## ADDED Requirements

### Requirement: query_nexus_health tool MUST exist

The system SHALL expose a `query_nexus_health` tool that calls `GetHealth` RPC on all
connected agents. Returns machine stats (CPU, memory, disk, load, uptime, docker containers, session count).

#### Scenario: Health query returns machine stats
Given "homelab" agent is connected
When Claude calls `query_nexus_health()`
Then the response includes CPU%, memory GB, disk GB, load averages, and uptime for "homelab"

### Requirement: query_nexus_projects tool MUST exist

The system SHALL expose a `query_nexus_projects` tool that calls `ListProjects` RPC on all
connected agents. Returns available projects per agent.

#### Scenario: List projects across agents
Given "homelab" has projects ["oo", "tc"] and "macbook" has ["nv", "nx"]
When Claude calls `query_nexus_projects()`
Then the response shows projects grouped by agent name

### Requirement: query_nexus_agents tool MUST exist

The system SHALL expose a `query_nexus_agents` tool that wraps the existing
`NexusClient::status_summary()` method. Returns connection status of all configured agents.

#### Scenario: Agent status shows connectivity
Given "homelab" is connected and "macbook" is disconnected
When Claude calls `query_nexus_agents()`
Then the response shows "homelab: connected" and "macbook: disconnected"
