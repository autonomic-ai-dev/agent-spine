use std::sync::Arc;
use tokio::sync::watch;

/// Propagates cancellation signals from signal handlers to long-running
/// operations (executors, servers).
///
/// A `CancelToken` wraps a `tokio::sync::watch` channel. When `cancel()` is
/// called, all watchers see `true` and can initiate graceful shutdown.
#[derive(Clone, Debug)]
pub struct CancelToken {
    sender: Arc<watch::Sender<bool>>,
    receiver: watch::Receiver<bool>,
}

impl CancelToken {
    /// Create a new (non-cancelled) token.
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        Self {
            sender: Arc::new(sender),
            receiver,
        }
    }

    /// Request cancellation. Once called, `is_cancelled()` returns `true`
    /// for this and all cloned tokens.
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }

    /// Returns `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Returns a clone of the receiver for use in async select! loops.
    pub fn watch(&self) -> watch::Receiver<bool> {
        self.receiver.clone()
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix;

    let mut term =
        unix::signal(unix::SignalKind::terminate()).expect("failed to install SIGTERM handler");
    let mut int =
        unix::signal(unix::SignalKind::interrupt()).expect("failed to install SIGINT handler");

    tokio::select! {
        _ = term.recv() => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown...");
        }
        _ = int.recv() => {
            tracing::info!("Received SIGINT, initiating graceful shutdown...");
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("Received Ctrl+C, initiating graceful shutdown...");
}

/// Set up OS signal handlers for termination signals (SIGINT / SIGTERM on
/// Unix, Ctrl+C on Windows).
///
/// When the signal is received, the provided `CancelToken` is triggered
/// and a message is logged. Returns a join handle for the signal task.
pub fn setup_signal_handler(cancel: CancelToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        wait_for_signal().await;
        cancel.cancel();
    })
}
