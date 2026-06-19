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

/// Set up OS signal handlers for SIGINT and SIGTERM.
///
/// When either signal is received, the provided `CancelToken` is triggered
/// and a message is logged. Returns a join handle for the signal task.
pub fn setup_signal_handler(cancel: CancelToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        let mut int = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler");

        tokio::select! {
            _ = term.recv() => {
                tracing::info!("Received SIGTERM, initiating graceful shutdown...");
            }
            _ = int.recv() => {
                tracing::info!("Received SIGINT, initiating graceful shutdown...");
            }
        }

        cancel.cancel();
    })
}
