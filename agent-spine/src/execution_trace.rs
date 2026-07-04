//! Publish workflow execution traces for agent-muscle LoRA training (Phase 5).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

/// NATS subject for completed workflow traces (Kernel V2 Phase 5).
pub const EXECUTION_COMPLETED_SUBJECT: &str = "autonomic.spine.execution.completed";

/// Training trace emitted when a workflow finishes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionTraceEvent {
    pub msg_id: String,
    pub execution_id: String,
    pub workflow_name: String,
    pub prompt: String,
    pub completion: String,
    pub reward: f64,
    pub success: bool,
}

impl ExecutionTraceEvent {
    pub fn from_payloads(
        execution_id: &str,
        workflow_name: &str,
        initial: &Value,
        final_payload: &Value,
        success: bool,
    ) -> Self {
        let reward = if success { 1.0 } else { 0.0 };
        Self {
            msg_id: format!("trace:{execution_id}"),
            execution_id: execution_id.to_string(),
            workflow_name: workflow_name.to_string(),
            prompt: extract_prompt(initial),
            completion: extract_completion(final_payload),
            reward,
            success,
        }
    }
}

pub fn extract_prompt(value: &Value) -> String {
    for key in [
        "prompt",
        "task",
        "instruction",
        "user_message",
        "description",
        "query",
    ] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    serde_json::to_string(value).unwrap_or_default()
}

pub fn extract_completion(value: &Value) -> String {
    for key in [
        "completion",
        "result",
        "output",
        "response",
        "answer",
        "summary",
    ] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    serde_json::to_string(value).unwrap_or_default()
}

/// Best-effort async publish; no-op when NATS is unavailable.
pub fn spawn_publish(event: ExecutionTraceEvent) {
    if agent_body_core::default_nats_url().is_none() {
        debug!("NATS unavailable; skipping execution trace publish");
        return;
    }
    tokio::spawn(async move {
        if let Err(err) = publish(&event).await {
            warn!(error = %err, execution_id = %event.execution_id, "execution trace publish failed");
        }
    });
}

async fn publish(event: &ExecutionTraceEvent) -> Result<(), String> {
    let client = agent_body_core::connect_nats()
        .await
        .map_err(|e| e.to_string())?;
    let js = crate::jetstream::ensure_autonomic_stream(&client).await?;
    let bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?;
    crate::jetstream::publish_dedup(
        &js,
        EXECUTION_COMPLETED_SUBJECT,
        &event.msg_id,
        &bytes,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_prompt_prefers_task_field() {
        let v = json!({"task": "fix CI", "output": "done"});
        assert_eq!(extract_prompt(&v), "fix CI");
    }

    #[test]
    fn extract_completion_prefers_result_field() {
        let v = json!({"task": "fix CI", "result": "tests pass"});
        assert_eq!(extract_completion(&v), "tests pass");
    }

    #[test]
    fn trace_event_reward_reflects_success() {
        let initial = json!({"prompt": "hello"});
        let final_p = json!({"completion": "world"});
        let ok = ExecutionTraceEvent::from_payloads("e1", "wf", &initial, &final_p, true);
        assert_eq!(ok.reward, 1.0);
        assert!(ok.success);
        let fail = ExecutionTraceEvent::from_payloads("e1", "wf", &initial, &final_p, false);
        assert_eq!(fail.reward, 0.0);
        assert!(!fail.success);
    }
}
