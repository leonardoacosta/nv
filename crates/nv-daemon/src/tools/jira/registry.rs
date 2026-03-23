//! Multi-instance Jira client registry.
//!
//! Holds a `HashMap<String, JiraClient>` keyed by instance name and routes
//! tool calls to the correct backend based on project KEY.

use std::collections::HashMap;

use anyhow::Result;
use nv_core::config::{JiraConfig, Secrets};

use super::client::JiraClient;

// ── JiraRegistry ────────────────────────────────────────────────────

/// Holds one `JiraClient` per configured Jira instance and routes tool
/// calls to the correct client by project KEY.
///
/// For backward-compatible flat configs, the map has a single entry keyed
/// `"default"` and every project resolves to that client.
pub struct JiraRegistry {
    /// Instance name → client.
    clients: HashMap<String, JiraClient>,
    /// Project KEY → instance name (from config's `project_map`).
    project_map: HashMap<String, String>,
    /// Snapshot of the config used to build this registry (for resolve fallback).
    config: JiraConfig,
}

impl JiraRegistry {
    /// Build a `JiraRegistry` from config + secrets.
    ///
    /// For flat configs, creates a single `"default"` client.
    /// For multi-instance configs, creates one client per instance that has
    /// credentials available.
    ///
    /// Returns `None` if no clients could be constructed (no credentials).
    pub fn new(config: &JiraConfig, secrets: &Secrets) -> Result<Option<Self>> {
        let mut clients = HashMap::new();
        let mut project_map = HashMap::new();

        match config {
            JiraConfig::Flat(cfg) => {
                // Flat config — single "default" instance
                let token = secrets.jira_token_for("default");
                let username = secrets.jira_username_for("default");
                match (username, token) {
                    (Some(u), Some(t)) => {
                        let url = format!("https://{}", cfg.instance);
                        clients.insert("default".to_string(), JiraClient::new(&url, u, t));
                        tracing::info!(
                            instance = %cfg.instance,
                            "Jira client configured (flat/default)"
                        );
                    }
                    _ => {
                        tracing::warn!("Jira configured but credentials missing — jira tools disabled");
                        return Ok(None);
                    }
                }
            }
            JiraConfig::Multi(multi) => {
                // Multi-instance — build one client per instance
                project_map = multi.project_map.clone();
                for (instance_name, cfg) in &multi.instances {
                    let token = secrets.jira_token_for(instance_name);
                    let username = secrets.jira_username_for(instance_name);
                    match (username, token) {
                        (Some(u), Some(t)) => {
                            let url = format!("https://{}", cfg.instance);
                            clients.insert(
                                instance_name.clone(),
                                JiraClient::new(&url, u, t),
                            );
                            tracing::info!(
                                instance_name = %instance_name,
                                host = %cfg.instance,
                                "Jira client configured"
                            );
                        }
                        _ => {
                            tracing::warn!(
                                instance_name = %instance_name,
                                "Jira instance configured but credentials missing — skipping"
                            );
                        }
                    }
                }
                if clients.is_empty() {
                    tracing::warn!("Jira multi-instance configured but no credentials found — jira tools disabled");
                    return Ok(None);
                }
            }
        }

        Ok(Some(Self {
            clients,
            project_map,
            config: config.clone(),
        }))
    }

    /// Resolve the correct `JiraClient` for a given project KEY.
    ///
    /// Resolution order:
    /// 1. `project_map` lookup → instance name → client
    /// 2. `default_project` match across all instances
    /// 3. `"default"` client (backward-compat flat config)
    /// 4. First client in the map
    pub fn resolve(&self, project: &str) -> Option<&JiraClient> {
        // 1. project_map
        if let Some(instance_name) = self.project_map.get(project) {
            if let Some(client) = self.clients.get(instance_name) {
                return Some(client);
            }
        }

        // 2. default_project match via config
        if let Some((instance_name, _)) = self.config.resolve_instance(project) {
            if let Some(client) = self.clients.get(instance_name) {
                return Some(client);
            }
        }

        // 3. "default" client
        if let Some(client) = self.clients.get("default") {
            return Some(client);
        }

        // 4. first client
        self.clients.values().next()
    }

    /// Resolve the correct `JiraClient` for an issue key (e.g. "OO-123").
    ///
    /// Extracts the project prefix before the first `-` and delegates to `resolve`.
    pub fn resolve_from_issue_key(&self, issue_key: &str) -> Option<&JiraClient> {
        let project = issue_key
            .split('-')
            .next()
            .unwrap_or(issue_key);
        self.resolve(project)
    }

    /// Return the `default_project` KEY from the first/default Jira instance config.
    /// Used as fallback when Claude omits the project field in `jira_create`.
    pub fn default_project(&self) -> Option<&str> {
        match &self.config {
            nv_core::config::JiraConfig::Flat(cfg) => Some(&cfg.default_project),
            nv_core::config::JiraConfig::Multi(multi) => {
                multi.instances.values().next().map(|cfg| cfg.default_project.as_str())
            }
        }
    }

    /// Return the `"default"` or first client, for call sites that don't have
    /// project context (e.g. backward-compatible callers).
    pub fn default_client(&self) -> Option<&JiraClient> {
        self.clients
            .get("default")
            .or_else(|| self.clients.values().next())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::{JiraInstanceConfig, JiraMultiConfig, Secrets};

    fn make_secrets_with(token: &str, username: &str) -> Secrets {
        Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: None,
            bluebubbles_password: None,
            ms_graph_client_id: None,
            ms_graph_client_secret: None,
            ms_graph_tenant_id: None,
            jira_api_token: Some(token.to_string()),
            jira_username: Some(username.to_string()),
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
            google_calendar_credentials: None,
        }
    }

    fn make_secrets_multi(
        tokens: &[(&str, &str)],
        usernames: &[(&str, &str)],
        default_token: Option<&str>,
        default_username: Option<&str>,
    ) -> Secrets {
        let mut jira_api_tokens = HashMap::new();
        let mut jira_usernames = HashMap::new();
        for (k, v) in tokens {
            jira_api_tokens.insert(k.to_uppercase(), v.to_string());
        }
        for (k, v) in usernames {
            jira_usernames.insert(k.to_uppercase(), v.to_string());
        }
        Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: None,
            bluebubbles_password: None,
            ms_graph_client_id: None,
            ms_graph_client_secret: None,
            ms_graph_tenant_id: None,
            jira_api_token: default_token.map(String::from),
            jira_username: default_username.map(String::from),
            elevenlabs_api_key: None,
            jira_api_tokens,
            jira_usernames,
            google_calendar_credentials: None,
        }
    }

    #[test]
    fn flat_config_default_project_returns_value() {
        let cfg = JiraConfig::Flat(JiraInstanceConfig {
            instance: "myteam.atlassian.net".to_string(),
            default_project: "OO".to_string(),
            webhook_secret: None,
        });
        let secrets = make_secrets_with("token", "user@example.com");
        let registry = JiraRegistry::new(&cfg, &secrets).unwrap().unwrap();
        assert_eq!(registry.default_project(), Some("OO"));
    }

    #[test]
    fn flat_config_creates_default_client() {
        let cfg = JiraConfig::Flat(JiraInstanceConfig {
            instance: "myteam.atlassian.net".to_string(),
            default_project: "OO".to_string(),
            webhook_secret: None,
        });
        let secrets = make_secrets_with("token", "user@example.com");
        let registry = JiraRegistry::new(&cfg, &secrets).unwrap().unwrap();
        assert!(registry.default_client().is_some());
        // Every project resolves to the same client
        assert!(registry.resolve("OO").is_some());
        assert!(registry.resolve("UNKNOWN").is_some());
    }

    #[test]
    fn flat_config_missing_credentials_returns_none() {
        let cfg = JiraConfig::Flat(JiraInstanceConfig {
            instance: "myteam.atlassian.net".to_string(),
            default_project: "OO".to_string(),
            webhook_secret: None,
        });
        let secrets = Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: None,
            bluebubbles_password: None,
            ms_graph_client_id: None,
            ms_graph_client_secret: None,
            ms_graph_tenant_id: None,
            jira_api_token: None,
            jira_username: None,
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
            google_calendar_credentials: None,
        };
        let registry = JiraRegistry::new(&cfg, &secrets).unwrap();
        assert!(registry.is_none());
    }

    #[test]
    fn multi_config_routes_by_project_map() {
        let mut instances = HashMap::new();
        instances.insert(
            "personal".to_string(),
            JiraInstanceConfig {
                instance: "personal.atlassian.net".to_string(),
                default_project: "OO".to_string(),
                webhook_secret: None,
            },
        );
        instances.insert(
            "llc".to_string(),
            JiraInstanceConfig {
                instance: "llc.atlassian.net".to_string(),
                default_project: "CT".to_string(),
                webhook_secret: None,
            },
        );
        let mut project_map = HashMap::new();
        project_map.insert("OO".to_string(), "personal".to_string());
        project_map.insert("TC".to_string(), "personal".to_string());
        project_map.insert("CT".to_string(), "llc".to_string());

        let cfg = JiraConfig::Multi(JiraMultiConfig {
            instances,
            project_map,
            webhook_secret: None,
        });
        let secrets = make_secrets_multi(
            &[("PERSONAL", "token-personal"), ("LLC", "token-llc")],
            &[("PERSONAL", "user@personal.com"), ("LLC", "user@llc.com")],
            None,
            None,
        );
        let registry = JiraRegistry::new(&cfg, &secrets).unwrap().unwrap();

        // Route OO -> personal client (base_url ends with personal.atlassian.net)
        let oo_client = registry.resolve("OO").unwrap();
        assert!(oo_client.base_url().contains("personal.atlassian.net"));

        // Route CT -> llc client
        let ct_client = registry.resolve("CT").unwrap();
        assert!(ct_client.base_url().contains("llc.atlassian.net"));
    }

    #[test]
    fn resolve_from_issue_key_extracts_project() {
        let cfg = JiraConfig::Flat(JiraInstanceConfig {
            instance: "myteam.atlassian.net".to_string(),
            default_project: "OO".to_string(),
            webhook_secret: None,
        });
        let secrets = make_secrets_with("token", "user@example.com");
        let registry = JiraRegistry::new(&cfg, &secrets).unwrap().unwrap();
        assert!(registry.resolve_from_issue_key("OO-123").is_some());
        assert!(registry.resolve_from_issue_key("TC-456").is_some());
    }
}
