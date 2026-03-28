//! PgPool — shared Postgres connection pool for the daemon.
//!
//! Provides a thin wrapper around `tokio_postgres::Client` behind `Arc` with:
//! - TLS detection (TLS for Neon/cloud, NoTls for local)
//! - Connection driver task spawn
//! - Reconnection on connection loss
//! - 5-second connection timeout
//!
//! This matches the existing pattern in `scheduler.rs` but is reusable across
//! all Postgres-writing stores (obligations, contacts, sessions, briefings).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio_postgres::Client;

/// Connection timeout for Postgres operations.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Shared Postgres connection pool.
///
/// Wraps a single `tokio_postgres::Client` behind `Arc<Mutex<>>`. When a query
/// finds the client disconnected, the next call to `client()` attempts
/// reconnection transparently.
#[derive(Clone)]
pub struct PgPool {
    inner: Arc<Mutex<Option<Client>>>,
    database_url: String,
    use_tls: bool,
}

impl PgPool {
    /// Connect to the Postgres database specified by `DATABASE_URL`.
    ///
    /// Returns `Ok(PgPool)` even if the initial connection fails — the daemon
    /// must not crash because Postgres is temporarily unavailable. In that case
    /// the inner client is `None` and will be lazily connected on first use.
    pub async fn connect(database_url: &str) -> Self {
        let use_tls =
            database_url.contains("sslmode=require") || database_url.contains("neon.tech");

        let pool = Self {
            inner: Arc::new(Mutex::new(None)),
            database_url: database_url.to_string(),
            use_tls,
        };

        // Attempt initial connection (best-effort — daemon starts regardless).
        match pool.try_connect().await {
            Ok(client) => {
                tracing::info!("pg_pool: connected to Postgres");
                *pool.inner.lock().await = Some(client);
            }
            Err(e) => {
                tracing::warn!(error = %e, "pg_pool: initial Postgres connection failed — will retry on first write");
            }
        }

        pool
    }

    /// Obtain a reference to the underlying client, reconnecting if necessary.
    ///
    /// Returns `None` if the connection cannot be established. Callers must
    /// handle this gracefully (log and continue — never crash).
    pub async fn client(&self) -> Option<ClientGuard<'_>> {
        let mut guard = self.inner.lock().await;

        // Check if the existing client is still alive.
        if let Some(ref client) = *guard {
            if !client.is_closed() {
                return Some(ClientGuard { _guard: guard });
            }
            tracing::warn!("pg_pool: connection lost — reconnecting");
        }

        // Reconnect.
        match self.try_connect().await {
            Ok(client) => {
                *guard = Some(client);
                tracing::info!("pg_pool: reconnected to Postgres");
                Some(ClientGuard { _guard: guard })
            }
            Err(e) => {
                tracing::warn!(error = %e, "pg_pool: reconnection failed");
                *guard = None;
                None
            }
        }
    }

    /// Low-level connection helper. Matches the TLS pattern from `scheduler.rs`.
    async fn try_connect(&self) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        if self.use_tls {
            let tls_config = rustls::ClientConfig::builder()
                .with_root_certificates({
                    let mut roots = rustls::RootCertStore::empty();
                    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                    roots
                })
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);

            let (client, connection) = tokio::time::timeout(
                CONNECT_TIMEOUT,
                tokio_postgres::connect(&self.database_url, tls),
            )
            .await??;

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::warn!(error = %e, "pg_pool: connection driver error (tls)");
                }
            });

            Ok(client)
        } else {
            let (client, connection) = tokio::time::timeout(
                CONNECT_TIMEOUT,
                tokio_postgres::connect(&self.database_url, tokio_postgres::NoTls),
            )
            .await??;

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::warn!(error = %e, "pg_pool: connection driver error (no-tls)");
                }
            });

            Ok(client)
        }
    }
}

/// RAII guard that holds the `MutexGuard` and provides access to the `Client`.
///
/// The caller interacts with the `Client` via `get()`. When the guard is
/// dropped the mutex is released.
pub struct ClientGuard<'a> {
    _guard: tokio::sync::MutexGuard<'a, Option<Client>>,
}

impl<'a> ClientGuard<'a> {
    /// Access the underlying `tokio_postgres::Client`.
    ///
    /// # Panics
    /// Panics if the guard was constructed without a valid client (should never
    /// happen — `PgPool::client()` only returns `Some(ClientGuard)` when the
    /// inner option is `Some`).
    pub fn get(&self) -> &Client {
        self._guard
            .as_ref()
            .expect("ClientGuard created with None client")
    }
}
