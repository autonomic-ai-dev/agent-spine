use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::oneshot;

struct PendingTask {
    sender: oneshot::Sender<Value>,
    payload: Value,
}

/// The Supervisor manages paused graph executions, delegating them to IDE agents.
#[derive(Default, Clone)]
pub struct Supervisor {
    pending: Arc<Mutex<HashMap<String, PendingTask>>>,
}

impl Supervisor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Suspend execution and wait for the IDE agent to provide the next payload.
    #[tracing::instrument(skip(self, payload), fields(node = %node_name))]
    pub async fn delegate(
        &self,
        node_name: String,
        payload: Value,
        timeout: Option<std::time::Duration>,
    ) -> Result<Value, SupervisorError> {
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().map_err(|_| SupervisorError::Poisoned)?;
            if pending.contains_key(&node_name) {
                return Err(SupervisorError::AlreadyPending(node_name));
            }
            pending.insert(
                node_name.clone(),
                PendingTask {
                    sender: tx,
                    payload,
                },
            );
            tracing::info!("Agent task suspended and waiting for delegation result");
        }

        if let Some(duration) = timeout {
            match tokio::time::timeout(duration, rx).await {
                Ok(Ok(res)) => Ok(res),
                Ok(Err(_)) => {
                    tracing::warn!("Agent channel dropped");
                    Err(SupervisorError::Dropped(node_name))
                }
                Err(_) => {
                    tracing::warn!("Agent task timed out after {} seconds", duration.as_secs());
                    if let Ok(mut pending) = self.pending.lock() {
                        pending.remove(&node_name);
                    }
                    Err(SupervisorError::Timeout(node_name))
                }
            }
        } else {
            tracing::info!(
                "Waiting indefinitely for human intervention on '{}'",
                node_name
            );
            match rx.await {
                Ok(res) => Ok(res),
                Err(_) => {
                    tracing::warn!("Agent channel dropped");
                    Err(SupervisorError::Dropped(node_name))
                }
            }
        }
    }

    /// Provide the result for a pending task, resuming its execution in the executor.
    #[tracing::instrument(skip(self, result), fields(node = %node_name))]
    pub fn resume(&self, node_name: &str, result: Value) -> Result<(), SupervisorError> {
        let task = {
            let mut pending = self.pending.lock().map_err(|_| SupervisorError::Poisoned)?;
            pending
                .remove(node_name)
                .ok_or_else(|| SupervisorError::NotPending(node_name.to_owned()))?
        };

        tracing::info!("Resuming agent task with external result");
        task.sender
            .send(result)
            .map_err(|_| SupervisorError::Dropped(node_name.to_owned()))
    }

    /// Auto-resolve a pending task with the given result payload.
    #[tracing::instrument(skip(self), fields(node = %node_name))]
    pub fn auto_resolve(&self, node_name: &str, result: Value) -> Result<(), SupervisorError> {
        let task = {
            let mut pending = self.pending.lock().map_err(|_| SupervisorError::Poisoned)?;
            pending
                .remove(node_name)
                .ok_or_else(|| SupervisorError::NotPending(node_name.to_owned()))?
        };

        tracing::info!("Auto-resolving agent task for '{}'", node_name);
        task.sender
            .send(result)
            .map_err(|_| SupervisorError::Dropped(node_name.to_owned()))
    }

    /// Get a list of currently pending tasks waiting for IDE intervention.
    #[must_use]
    pub fn pending_tasks(&self) -> Vec<String> {
        self.pending
            .lock()
            .map(|guard| guard.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the stored payload for a pending task, if available.
    #[must_use]
    pub fn pending_payload(&self, node_name: &str) -> Option<Value> {
        self.pending
            .lock()
            .ok()
            .and_then(|guard| guard.get(node_name).map(|t| t.payload.clone()))
    }
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("task for node '{0}' is already pending")]
    AlreadyPending(String),
    #[error("no pending task for node '{0}'")]
    NotPending(String),
    #[error("the execution channel for node '{0}' was dropped")]
    Dropped(String),
    #[error("the execution channel for node '{0}' timed out")]
    Timeout(String),
    #[error("supervisor lock is poisoned")]
    Poisoned,
}
