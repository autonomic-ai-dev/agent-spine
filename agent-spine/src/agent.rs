use std::time::Duration;

use serde_json::{Value, json};
use tracing;

use crate::supervisor::Supervisor;

/// A built-in local agent that auto-resolves supervisor tasks.
///
/// Agent nodes pass through with `_node` metadata added.
/// ApprovalGate nodes auto-approve by default.
/// Runs as a background tokio task.
pub struct LocalAgent {
    supervisor: Supervisor,
    poll_interval: Duration,
    auto_approve: bool,
}

impl LocalAgent {
    /// Create a new local agent with the given supervisor.
    #[must_use]
    pub fn new(supervisor: Supervisor) -> Self {
        Self {
            supervisor,
            poll_interval: Duration::from_millis(100),
            auto_approve: true,
        }
    }

    /// Set the poll interval for checking pending tasks.
    #[must_use]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set whether ApprovalGate nodes should be auto-approved.
    #[must_use]
    pub fn with_auto_approve(mut self, approve: bool) -> Self {
        self.auto_approve = approve;
        self
    }

    /// Spawn the local agent as a background tokio task.
    pub fn spawn(self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&self) {
        loop {
            let pending = self.supervisor.pending_tasks();
            for task_name in pending {
                if let Some(payload) = self.supervisor.pending_payload(&task_name) {
                    let has_approved =
                        payload.get("approved").and_then(Value::as_bool) == Some(true);
                    let is_gate = task_name.contains("gate") || task_name.contains("approve");

                    if is_gate && !self.auto_approve && !has_approved {
                        tracing::debug!(
                            "LocalAgent: skipping approval gate '{}' (auto_approve=false)",
                            task_name
                        );
                        continue;
                    }

                    let result = if is_gate {
                        tracing::info!("LocalAgent: auto-approving '{}'", task_name);
                        let mut p = payload.clone();
                        if let Some(obj) = p.as_object_mut() {
                            obj.insert("approved".into(), json!(true));
                        }
                        p
                    } else {
                        tracing::info!("LocalAgent: processing '{}'", task_name);
                        let mut p = payload.clone();
                        if let Some(obj) = p.as_object_mut() {
                            obj.insert("_node".into(), json!(task_name));
                            obj.insert("_status".into(), json!("completed"));
                        }
                        p
                    };

                    if let Err(e) = self.supervisor.auto_resolve(&task_name, result) {
                        tracing::debug!("LocalAgent: failed to resolve '{}': {e}", task_name);
                    }
                }
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}
