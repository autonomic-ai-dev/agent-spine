//! Node retry backoff, circuit breaking, and dead-letter queue publishing.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use agent_body_core::nats::subjects;

pub use crate::workflow::DEFAULT_NODE_TIMEOUT_SECS;

/// Exponential backoff: `base_ms * 2^attempt`.
#[must_use]
pub fn exponential_backoff_ms(base_ms: u64, attempt: u32) -> u64 {
    base_ms.saturating_mul(2u64.saturating_pow(attempt))
}

/// Simple per-node circuit breaker — opens after `threshold` consecutive failures.
#[derive(Debug)]
pub struct CircuitBreaker {
    failures: AtomicU32,
    threshold: u32,
    open: AtomicBool,
}

impl CircuitBreaker {
    #[must_use]
    pub fn new(threshold: u32) -> Self {
        Self {
            failures: AtomicU32::new(0),
            threshold: threshold.max(1),
            open: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn shared(threshold: u32) -> Arc<Self> {
        Arc::new(Self::new(threshold))
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    /// Record a failure. Returns `true` if the circuit is now open.
    pub fn record_failure(&self) -> bool {
        let count = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.threshold {
            self.open.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.open.store(false, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DlqEntry {
    pub execution_id: String,
    pub workflow_name: String,
    pub node_name: String,
    pub node_kind: String,
    pub error: String,
    pub attempts: u32,
    pub payload: Value,
    pub failed_at: String,
}

/// Publish a failed workflow node to `system.dlq` for operator review/replay.
#[cfg(feature = "nats")]
pub async fn publish_dlq(entry: &DlqEntry) -> Result<(), String> {
    let client = agent_body_core::connect_nats()
        .await
        .map_err(|e| format!("dlq nats connect: {e}"))?;
    let js = crate::jetstream::ensure_autonomic_stream(&client)
        .await
        .map_err(|e| format!("dlq stream ensure: {e}"))?;
    let msg_id = format!(
        "dlq-{}-{}",
        entry.execution_id,
        entry.node_name.replace(' ', "_")
    );
    let bytes = serde_json::to_vec(entry).map_err(|e| format!("dlq serialize: {e}"))?;
    crate::jetstream::publish_dedup(&js, subjects::SYSTEM_DLQ, &msg_id, &bytes).await
}

#[cfg(not(feature = "nats"))]
pub async fn publish_dlq(_entry: &DlqEntry) -> Result<(), String> {
    Err("NATS feature disabled; DLQ not available".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_each_attempt() {
        assert_eq!(exponential_backoff_ms(100, 0), 100);
        assert_eq!(exponential_backoff_ms(100, 1), 200);
        assert_eq!(exponential_backoff_ms(100, 2), 400);
    }

    #[test]
    fn circuit_opens_at_threshold() {
        let cb = CircuitBreaker::new(3);
        assert!(!cb.record_failure());
        assert!(!cb.record_failure());
        assert!(cb.record_failure());
        assert!(cb.is_open());
        cb.record_success();
        assert!(!cb.is_open());
    }
}
