//! `Checkable` trait implementations for nv-tools service clients.
//!
//! These live in the daemon (not in nv-tools) because `Checkable`, `CheckResult`,
//! and `check::timed` are daemon-specific types.

use nv_tools::tools::{
    ado::AdoClient,
    calendar::CalendarClient,
    cloudflare::CloudflareClient,
    docker::DockerClient,
    doppler::DopplerClient,
    github::GithubClient,
    ha::HAClient,
    neon::NeonClient,
    plaid::PlaidClient,
    posthog::PosthogClient,
    resend::ResendClient,
    sentry::SentryClient,
    stripe::StripeClient,
    upstash::UpstashClient,
    vercel::VercelClient,
};

use super::{CheckResult, Checkable};

// ── Stripe ────────────────────────────────────────────────────────────

const STRIPE_BASE_URL: &str = "https://api.stripe.com/v1";

#[async_trait::async_trait]
impl Checkable for StripeClient {
    fn name(&self) -> &str {
        "stripe"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http_client()
                .get(format!("{STRIPE_BASE_URL}/balance"))
                .send()
                .await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "balance endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "invalid API key (401) — check STRIPE_SECRET_KEY".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }

    async fn check_write(&self) -> Option<CheckResult> {
        use super::check::timed;
        // POST /v1/invoices with no body — expect 400 (missing required fields), not 2xx
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http_client()
                .post(format!("{STRIPE_BASE_URL}/invoices"))
                .send()
                .await
        })
        .await;
        let result = match result {
            Ok(resp) if resp.status().as_u16() == 400 => CheckResult::Healthy {
                latency_ms: latency,
                detail: "invoices endpoint writable (400 as expected)".into(),
            },
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "invoices endpoint writable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "write probe: invalid API key (401)".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("write probe: HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: format!("write probe: {e}"),
            },
        };
        Some(result)
    }
}

// ── Vercel ────────────────────────────────────────────────────────────

const VERCEL_API: &str = "https://api.vercel.com";

#[async_trait::async_trait]
impl Checkable for VercelClient {
    fn name(&self) -> &str {
        "vercel"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.get(&format!("{VERCEL_API}/v2/user")).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "user endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "token expired or invalid (401) — check VERCEL_TOKEN".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Sentry ────────────────────────────────────────────────────────────

const SENTRY_BASE_URL: &str = "https://sentry.io/api/0";

#[async_trait::async_trait]
impl Checkable for SentryClient {
    fn name(&self) -> &str {
        "sentry"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{SENTRY_BASE_URL}/organizations/{}/", self.org);
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: format!("org: {}", self.org),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "token invalid or expired (401) — check SENTRY_AUTH_TOKEN".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => CheckResult::Unhealthy {
                error: format!("access denied (403) to org '{}' — check token scopes", self.org),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Resend ────────────────────────────────────────────────────────────

const RESEND_BASE_URL: &str = "https://api.resend.com";

#[async_trait::async_trait]
impl Checkable for ResendClient {
    fn name(&self) -> &str {
        "resend"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{RESEND_BASE_URL}/domains");
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "domains endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "API key invalid (401) — check RESEND_API_KEY".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Upstash ────────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for UpstashClient {
    fn name(&self) -> &str {
        "upstash"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.execute_command(&["INFO"]).await
        })
        .await;
        match result {
            Ok(_) => CheckResult::Healthy {
                latency_ms: latency,
                detail: "INFO command succeeded".into(),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Home Assistant ─────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for HAClient {
    fn name(&self) -> &str {
        "ha"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{}/api/", self.base_url);
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: format!("API reachable ({})", self.base_url),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "token invalid (401) — check HA_TOKEN".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: format!("unreachable ({}): {e}", self.base_url),
            },
        }
    }

    async fn check_write(&self) -> Option<CheckResult> {
        use super::check::timed;
        let url = format!("{}/api/services/light/turn_on", self.base_url);
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http.post(&url).json(&serde_json::json!({})).send().await
        })
        .await;
        let result = match result {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 400 => {
                CheckResult::Healthy {
                    latency_ms: latency,
                    detail: "services endpoint writable".into(),
                }
            }
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "write probe: token invalid (401)".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("write probe: HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: format!("write probe: {e}"),
            },
        };
        Some(result)
    }
}

// ── Azure DevOps ───────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for AdoClient {
    fn name(&self) -> &str {
        "ado"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{}/_apis/projects?api-version=7.1", self.org_url);
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.http.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "projects endpoint reachable".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "PAT invalid or expired (401) — check ADO_PAT".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => CheckResult::Unhealthy {
                error: "PAT lacks read permission (403) — check ADO_PAT scopes".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Plaid ──────────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for PlaidClient {
    fn name(&self) -> &str {
        "plaid"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;

        if std::env::var("PLAID_DB_URL").is_err() {
            return CheckResult::Missing {
                env_var: "PLAID_DB_URL".into(),
            };
        }

        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            match nv_tools::tools::plaid::connect_for_check().await {
                Ok(client) => client
                    .query_one("SELECT 1", &[])
                    .await
                    .map(|_| ())
                    .map_err(|e| anyhow::anyhow!(e)),
                Err(e) => Err(e),
            }
        })
        .await;

        match result {
            Ok(_) => CheckResult::Healthy {
                latency_ms: latency,
                detail: "cortex-postgres reachable (SELECT 1 ok)".into(),
            },
            Err(e) => CheckResult::Unhealthy {
                error: format!("connection failed: {e}"),
            },
        }
    }
}

// ── Doppler ────────────────────────────────────────────────────────────

const DOPPLER_API: &str = "https://api.doppler.com";

#[async_trait::async_trait]
impl Checkable for DopplerClient {
    fn name(&self) -> &str {
        "doppler"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{DOPPLER_API}/v3/me");
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "authenticated (v3/me ok)".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "token invalid or expired (401) — check DOPPLER_API_TOKEN".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Cloudflare ────────────────────────────────────────────────────────

const CF_API: &str = "https://api.cloudflare.com/client/v4";

#[async_trait::async_trait]
impl Checkable for CloudflareClient {
    fn name(&self) -> &str {
        "cloudflare"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let url = format!("{CF_API}/user/tokens/verify");
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            self.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: "token verified".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "token invalid (401) — check CLOUDFLARE_API_TOKEN".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => CheckResult::Unhealthy {
                error: "token lacks Zone:Read permission (403)".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── PostHog ───────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for PosthogClient {
    fn name(&self) -> &str {
        "posthog"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;

        let key = match nv_tools::tools::posthog::api_key_pub() {
            Ok(k) => k,
            Err(_) => {
                return CheckResult::Missing {
                    env_var: "POSTHOG_API_KEY".into(),
                }
            }
        };

        let h = nv_tools::tools::posthog::host_pub();
        let url = format!("https://{h}/api/projects/");

        let client = match nv_tools::tools::posthog::build_client_pub(&key) {
            Ok(c) => c,
            Err(e) => {
                return CheckResult::Unhealthy {
                    error: format!("failed to build client: {e}"),
                }
            }
        };

        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            client.get(&url).send().await
        })
        .await;
        match result {
            Ok(resp) if resp.status().is_success() => CheckResult::Healthy {
                latency_ms: latency,
                detail: format!("projects endpoint reachable ({h})"),
            },
            Ok(resp) if resp.status().as_u16() == 401 => CheckResult::Unhealthy {
                error: "API key invalid (401) — check POSTHOG_API_KEY".into(),
            },
            Ok(resp) => CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}

// ── Neon ──────────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for NeonClient {
    fn name(&self) -> &str {
        "neon"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;

        let env_key = format!("POSTGRES_URL_{}", self.project.to_uppercase());
        if std::env::var(&env_key).is_err() {
            return CheckResult::Missing { env_var: env_key };
        }

        let project = self.project.clone();
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            nv_tools::tools::neon::connect_for_check(&project).await
        })
        .await;

        match result {
            Ok(client) => match client.query_one("SELECT 1", &[]).await {
                Ok(_) => CheckResult::Healthy {
                    latency_ms: latency,
                    detail: format!("SELECT 1 ok ({})", self.project.to_uppercase()),
                },
                Err(e) => CheckResult::Unhealthy {
                    error: format!("query failed: {e}"),
                },
            },
            Err(e) => CheckResult::Unhealthy {
                error: format!("connection failed: {e}"),
            },
        }
    }
}

// ── Docker ────────────────────────────────────────────────────────────

const DOCKER_TIMEOUT_SECS: u64 = 10;

#[async_trait::async_trait]
impl Checkable for DockerClient {
    fn name(&self) -> &str {
        "docker"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            tokio::time::timeout(
                std::time::Duration::from_secs(DOCKER_TIMEOUT_SECS),
                tokio::process::Command::new("docker")
                    .arg("info")
                    .arg("--format")
                    .arg("{{.ServerVersion}}")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .output(),
            )
            .await
        })
        .await;
        match result {
            Ok(Ok(output)) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                CheckResult::Healthy {
                    latency_ms: latency,
                    detail: format!("docker daemon reachable (v{version})"),
                }
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                CheckResult::Unhealthy {
                    error: if stderr.is_empty() {
                        format!("docker info exited with code {:?}", output.status.code())
                    } else {
                        stderr
                    },
                }
            }
            Ok(Err(e)) => CheckResult::Unhealthy {
                error: format!("failed to run docker: {e}"),
            },
            Err(_) => CheckResult::Unhealthy {
                error: format!("docker info timed out after {DOCKER_TIMEOUT_SECS}s"),
            },
        }
    }
}

// ── GitHub ────────────────────────────────────────────────────────────

const GH_CHECK_TIMEOUT_SECS: u64 = 15;

#[async_trait::async_trait]
impl Checkable for GithubClient {
    fn name(&self) -> &str {
        "github"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;
        let (latency, result) = timed(std::time::Duration::from_secs(15), || async {
            tokio::time::timeout(
                std::time::Duration::from_secs(GH_CHECK_TIMEOUT_SECS),
                tokio::process::Command::new("gh")
                    .args(["auth", "status"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output(),
            )
            .await
        })
        .await;
        match result {
            Ok(Ok(output)) if output.status.success() => {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                let detail = combined
                    .lines()
                    .find(|l| l.contains("Logged in"))
                    .map(|l| l.trim().to_string())
                    .unwrap_or_else(|| "gh auth status ok".into());
                CheckResult::Healthy {
                    latency_ms: latency,
                    detail,
                }
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                CheckResult::Unhealthy {
                    error: if stderr.is_empty() {
                        "gh auth status failed — run `gh auth login`".into()
                    } else {
                        stderr
                    },
                }
            }
            Ok(Err(e)) => CheckResult::Unhealthy {
                error: format!("failed to run gh: {e}"),
            },
            Err(_) => CheckResult::Unhealthy {
                error: format!("gh auth status timed out after {GH_CHECK_TIMEOUT_SECS}s"),
            },
        }
    }
}

// ── Calendar ──────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl Checkable for CalendarClient {
    fn name(&self) -> &str {
        "calendar"
    }

    async fn check_read(&self) -> CheckResult {
        use super::check::timed;

        let (latency, result): (u64, anyhow::Result<Vec<nv_tools::tools::calendar::Event>>) =
            timed(std::time::Duration::from_secs(15), || async {
                let now = chrono::Utc::now();
                let time_min = now.to_rfc3339();
                self.query_events(&[
                    ("timeMin", time_min.as_str()),
                    ("singleEvents", "true"),
                    ("orderBy", "startTime"),
                    ("maxResults", "1"),
                ])
                .await
            })
            .await;

        match result {
            Ok(events) => {
                let detail = if events.is_empty() {
                    "no upcoming events".to_string()
                } else {
                    events
                        .first()
                        .and_then(|e| e.summary.clone())
                        .unwrap_or_else(|| "next event found".to_string())
                };
                CheckResult::Healthy { latency_ms: latency, detail }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not configured") || msg.contains("not set") {
                    CheckResult::Missing {
                        env_var: "GOOGLE_CALENDAR_CREDENTIALS".into(),
                    }
                } else {
                    CheckResult::Unhealthy { error: msg }
                }
            }
        }
    }
}
