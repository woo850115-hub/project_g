use tokio::sync::watch;

/// Sender side — held by main, triggers shutdown.
#[derive(Clone)]
pub struct ShutdownTx(watch::Sender<bool>);

/// Receiver side — cloned to each subsystem.
#[derive(Clone)]
pub struct ShutdownRx(watch::Receiver<bool>);

/// Create a shutdown channel pair.
pub fn shutdown_channel() -> (ShutdownTx, ShutdownRx) {
    let (tx, rx) = watch::channel(false);
    (ShutdownTx(tx), ShutdownRx(rx))
}

impl ShutdownTx {
    /// Signal all receivers to shut down.
    pub fn trigger(&self) {
        let _ = self.0.send(true);
    }
}

impl ShutdownRx {
    /// Async wait until shutdown is signaled.
    pub async fn wait(&mut self) {
        while !*self.0.borrow() {
            if self.0.changed().await.is_err() {
                return; // sender dropped
            }
        }
    }

    /// Non-blocking check (for tick loop polling).
    pub fn is_shutdown(&self) -> bool {
        *self.0.borrow()
    }

    /// Unwrap into the underlying watch::Receiver for passing to external crates.
    pub fn into_inner(self) -> watch::Receiver<bool> {
        self.0
    }
}

/// Wait for SIGINT or SIGTERM (Unix) or Ctrl+C (all platforms).
pub async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).expect("failed to register SIGINT");
        let mut sigterm = signal(SignalKind::terminate()).expect("failed to register SIGTERM");
        tokio::select! {
            _ = sigint.recv() => { tracing::info!("Received SIGINT"); }
            _ = sigterm.recv() => { tracing::info!("Received SIGTERM"); }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("Received Ctrl+C");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_channel_default_not_shutdown() {
        let (_tx, rx) = shutdown_channel();
        assert!(!rx.is_shutdown());
    }

    #[test]
    fn shutdown_channel_trigger() {
        let (tx, rx) = shutdown_channel();
        tx.trigger();
        assert!(rx.is_shutdown());
    }

    #[tokio::test]
    async fn shutdown_channel_async_wait() {
        let (tx, mut rx) = shutdown_channel();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            tx.trigger();
        });
        rx.wait().await;
        assert!(rx.is_shutdown());
    }

    #[test]
    fn shutdown_rx_clone() {
        let (tx, rx) = shutdown_channel();
        let rx2 = rx.clone();
        assert!(!rx2.is_shutdown());
        tx.trigger();
        assert!(rx.is_shutdown());
        assert!(rx2.is_shutdown());
    }
}
