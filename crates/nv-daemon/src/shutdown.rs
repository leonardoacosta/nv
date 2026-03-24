use tokio::signal::unix::{signal, SignalKind};

/// Wait for a shutdown signal (SIGTERM or Ctrl+C).
///
/// Returns when either signal is received. The caller is responsible
/// for orchestrating the actual shutdown sequence (draining channels,
/// saving state, etc.).
pub async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl+c handler");
    };

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("received ctrl+c, initiating shutdown");
        }
        () = sigterm => {
            tracing::info!("received SIGTERM, initiating shutdown");
        }
    }
}

// drain_with_timeout was removed: no mpsc channel is in scope at the shutdown
// callsite in main.rs — the orchestrator handles its own channel draining.
// If a trigger channel is added to the shutdown path in future, this utility
// can be reinstated from git history.
