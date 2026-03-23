# Implementation Tasks

<!-- beads:epic:TBD -->

## Config Layer (nv-core)

- [x] [1.1] [P-1] Add `JiraInstanceConfig` struct (instance, default_project, webhook_secret) and refactor `JiraConfig` to support both flat single-instance and nested multi-instance formats via serde untagged enum or custom deserializer [owner:api-engineer]
- [x] [1.2] [P-1] Add `project_map: HashMap<String, String>` field to `JiraConfig` (project code -> instance name, default empty) [owner:api-engineer]
- [x] [1.3] [P-2] Add `JiraConfig::resolve_instance(&self, project: &str) -> Option<(&str, &JiraInstanceConfig)>` — lookup chain: project_map -> default_project match -> "default" -> first [owner:api-engineer]
- [x] [1.4] [P-2] Extend `Secrets` to load instance-qualified env vars: `JIRA_API_TOKEN_{INSTANCE}`, `JIRA_USERNAME_{INSTANCE}` with fallback to unqualified vars [owner:api-engineer]
- [x] [1.5] [P-2] Add config tests: parse flat format, parse multi-instance format, parse mixed (flat with project_map — should error or warn), backward compatibility with existing nv.toml [owner:api-engineer]

## JiraRegistry (nv-daemon)

- [x] [2.1] [P-1] Create `JiraRegistry` struct in `jira/mod.rs` — holds `HashMap<String, JiraClient>`, `project_map: HashMap<String, String>`, and instance configs [owner:api-engineer]
- [x] [2.2] [P-1] Add `JiraRegistry::new(config: &JiraConfig, secrets: &Secrets) -> Result<Self>` — iterates instances, loads per-instance credentials, builds clients [owner:api-engineer]
- [x] [2.3] [P-2] Add `JiraRegistry::resolve(&self, project: &str) -> Option<&JiraClient>` — resolves project to instance name via project_map -> default_project match -> "default" -> first [owner:api-engineer]
- [x] [2.4] [P-2] Add `JiraRegistry::resolve_from_issue_key(&self, issue_key: &str) -> Option<&JiraClient>` — extracts project prefix from key (e.g. "OO-123" -> "OO") and delegates to resolve() [owner:api-engineer]
- [x] [2.5] [P-2] Add `JiraRegistry::default_client(&self) -> Option<&JiraClient>` — returns "default" or first client (for backward compat call sites that don't have project context) [owner:api-engineer]

## Call Site Migration (nv-daemon)

- [x] [3.1] [P-1] Update `SharedDeps` in worker.rs — change `jira_client: Option<JiraClient>` to `jira_registry: Option<JiraRegistry>` [owner:api-engineer]
- [x] [3.2] [P-1] Update main.rs — build `JiraRegistry` from config + secrets instead of single `JiraClient`; pass to `SharedDeps` [owner:api-engineer]
- [x] [3.3] [P-2] Update `execute_tool` and `execute_tool_v2` in tools.rs — change `jira_client: Option<&JiraClient>` param to `jira_registry: Option<&JiraRegistry>`; route jira_search/jira_get by extracting project from input or issue_key [owner:api-engineer]
- [x] [3.4] [P-2] Update `execute_jira_action` in tools.rs — accept `&JiraRegistry` instead of `&JiraClient`; resolve client from payload's project/issue_key [owner:api-engineer]
- [x] [3.5] [P-2] Update agent.rs AgentLoop — change `jira_client` field to `jira_registry`, update all references [owner:api-engineer]
- [x] [3.6] [P-2] Update callbacks.rs `handle_approve` — accept `Option<&JiraRegistry>`, resolve client from action payload [owner:api-engineer]
- [x] [3.7] [P-2] Update aggregation.rs `project_health` — accept `Option<&JiraRegistry>`, resolve client by project code [owner:api-engineer]
- [x] [3.8] [P-2] Update worker.rs tool execution call sites to pass `jira_registry.as_ref()` [owner:api-engineer]

## Config Documentation

- [x] [4.1] [P-2] Update `config/nv.toml` — add commented multi-instance Jira example section below existing `[jira]` block [owner:api-engineer]

## Verify

- [x] [5.1] cargo build passes [owner:api-engineer]
- [x] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [5.3] cargo test — new config parsing tests (flat, multi-instance, backward compat, resolve chain) + existing tests pass [owner:api-engineer]
