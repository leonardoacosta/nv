use std::time::Duration;

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

/// Drain remaining items from an mpsc receiver with a timeout.
///
/// Returns the number of items drained.
#[allow(dead_code)]
pub async fn drain_with_timeout<T>(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<T>,
    timeout: Duration,
) -> usize {
    let mut count = 0;
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            item = rx.recv() => {
                match item {
                    Some(_) => count += 1,
                    None => break, // channel closed
                }
            }
            () = &mut deadline => {
                tracing::warn!(drained = count, "drain timeout reached");
                break;
            }
        }
    }

    count
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn drain_with_timeout_empty_channel() {
        let (_tx, mut rx) = mpsc::unbounded_channel::<u32>();
        drop(_tx); // close the channel
        let count = drain_with_timeout(&mut rx, Duration::from_millis(100)).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn drain_with_timeout_some_items() {
        let (tx, mut rx) = mpsc::unbounded_channel::<u32>();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        drop(tx);
        let count = drain_with_timeout(&mut rx, Duration::from_millis(100)).await;
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn drain_with_timeout_hits_timeout() {
        // Channel stays open (sender not dropped), so drain should hit timeout
        let (_tx, mut rx) = mpsc::unbounded_channel::<u32>();
        let count = drain_with_timeout(&mut rx, Duration::from_millis(50)).await;
        assert_eq!(count, 0);
    }
}
