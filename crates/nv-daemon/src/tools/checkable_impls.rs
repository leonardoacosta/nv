//! `Checkable` trait implementations for nv-tools service clients.
//!
//! These live in the daemon (not in nv-tools) because `Checkable`, `CheckResult`,
//! and `check::timed` are daemon-specific types.

use nv_tools::tools::{
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
