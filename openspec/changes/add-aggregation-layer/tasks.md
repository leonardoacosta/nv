# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/aggregation.rs — AggregationService struct holding Arc refs to all data source clients (VercelClient, SentryClient, JiraClient, NexusClient, NeonClient, GhClient, DockerClient, TailscaleClient, HAClient, PlaidClient, StripeClient) [owner:api-engineer]
- [ ] [1.2] [P-1] Define ProjectResources struct — vercel_project, sentry_slug, jira_key, github_repo, neon_project_id (all Option) [owner:api-engineer]
- [ ] [1.3] [P-1] Define PROJECT_MAP: HashMap/phf of project code -> ProjectResources for all known projects (oo, tc, tl, mv, ss, cl, co, cw, etc.) [owner:api-engineer]
- [ ] [1.4] [P-2] Implement project_health(code: &str) method — lookup PROJECT_MAP, spawn parallel tokio::join! calls for each available source, wrap each in 5s timeout, collect results [owner:api-engineer]
- [ ] [1.5] [P-2] Add format_project_health() helper — per-dimension status line (Deploy, Errors, Issues, Sessions, DB, CI) with unavailable fallback [owner:api-engineer]
- [ ] [1.6] [P-2] Implement homelab_status() method — tokio::join! on docker_status + tailscale_status + ha_states, each with 5s timeout [owner:api-engineer]
- [ ] [1.7] [P-2] Add format_homelab_status() helper — Docker container summary, Tailscale node summary, HA entity summary [owner:api-engineer]
- [ ] [1.8] [P-2] Implement financial_summary() method — tokio::join! on plaid_balances + stripe_invoices, each with 5s timeout [owner:api-engineer]
- [ ] [1.9] [P-2] Add format_financial_summary() helper — account balances + Stripe open invoices [owner:api-engineer]
- [ ] [1.10] [P-3] Add mod aggregation declaration in main.rs [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Register project_health tool in register_tools() — input schema: { code: string } [owner:api-engineer]
- [ ] [2.2] [P-1] Register homelab_status tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.3] [P-1] Register financial_summary tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.4] [P-2] Add dispatch cases in execute_tool() for all 3 tools — call AggregationService methods, return formatted output [owner:api-engineer]
- [ ] [2.5] [P-2] Init AggregationService in main.rs with Arc refs to all initialized data source clients [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — AggregationService tests: project_health with mocked sources (all succeed, partial fail, all fail), homelab_status with mocked sources, financial_summary with mocked sources + existing tests pass [owner:api-engineer]
- [ ] [3.4] cargo test — verify partial failure: one source times out, others return, output includes "[source]: unavailable" [owner:api-engineer]
- [ ] [3.5] [user] Manual test: ask Nova "How's OO?" via Telegram, verify project_health returns multi-dimension summary [owner:api-engineer]
- [ ] [3.6] [user] Manual test: ask Nova "Homelab status" via Telegram, verify Docker + Tailscale + HA combined output [owner:api-engineer]
- [ ] [3.7] [user] Manual test: ask Nova "Financial summary" via Telegram, verify Plaid + Stripe combined output with no PII [owner:api-engineer]
