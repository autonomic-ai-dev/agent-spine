use std::sync::{Arc, Mutex};

use serde_json::Value;
use thiserror::Error;

use crate::router::{ConfidenceRouter, RouterAction};
use crate::state::StateError;
use crate::supervisor::Supervisor;
use crate::workflow::NodeKind;
use crate::{ExecutionId, StateSnapshot, Transition, ValidatedWorkflow, WorkflowState};

/// Helper to deeply merge two JSON values.
fn merge_json(a: &mut Value, b: &Value) {
    if a.is_object() && b.is_object() {
        if let (Some(a_obj), Some(b_obj)) = (a.as_object_mut(), b.as_object()) {
            for (k, v) in b_obj {
                if let Some(a_v) = a_obj.get_mut(k) {
                    merge_json(a_v, v);
                } else {
                    a_obj.insert(k.clone(), v.clone());
                }
            }
        }
    } else {
        *a = b.clone();
    }
}

/// Orchestrates the execution of a `ValidatedWorkflow`.
pub struct Executor<S: WorkflowState> {
    workflow: ValidatedWorkflow,
    state_store: Arc<Mutex<S>>,
    supervisor: Supervisor,
    router: ConfidenceRouter,
}

impl<S: WorkflowState> Executor<S> {
    /// Create a new executor for the given workflow.
    pub const fn new(
        workflow: ValidatedWorkflow,
        state_store: Arc<Mutex<S>>,
        supervisor: Supervisor,
        router: ConfidenceRouter,
    ) -> Self {
        Self {
            workflow,
            state_store,
            supervisor,
            router,
        }
    }

    /// Execute the workflow from start to finish.
    pub async fn run(&mut self, initial_payload: Value) -> Result<ExecutionId, ExecutorError> {
        let execution_id = ExecutionId::new();
        let mut current_snapshot = StateSnapshot::initial(execution_id, initial_payload);

        // Persist initial state
        {
            let mut store = self
                .state_store
                .lock()
                .map_err(|_| ExecutorError::PoisonedLock)?;
            store
                .append(current_snapshot.clone())
                .map_err(ExecutorError::State)?;
        }

        let definition = self.workflow.definition();
        let nodes = definition.nodes();
        let edges = definition.edges();

        let mut current_node_names = vec![definition.start_node().to_owned()];

        // State machine execution
        loop {
            // Deduplicate to prevent double-execution of the same node in a level
            current_node_names.sort();
            current_node_names.dedup();

            let mut join_set = tokio::task::JoinSet::new();

            for node_name in &current_node_names {
                let node_name = node_name.clone();
                let node = nodes
                    .iter()
                    .find(|n| n.name() == node_name)
                    .expect("node must exist in workflow");

                let node_kind = node.kind().clone();
                let supervisor = self.supervisor.clone();
                let payload = current_snapshot.payload().clone();

                join_set.spawn(async move {
                    // Execute node based on kind
                    let next_payload = match node_kind {
                        NodeKind::Agent => supervisor
                            .delegate(node_name.clone(), payload)
                            .await
                            .map_err(|_| ExecutorError::SupervisorFailed)?,
                        NodeKind::Verify => payload,
                        NodeKind::Checkpoint => supervisor
                            .delegate(node_name.clone(), payload)
                            .await
                            .map_err(|_| ExecutorError::SupervisorFailed)?,
                    };
                    Ok::<_, ExecutorError>((node_name, next_payload))
                });
            }

            let mut branch_results = Vec::new();
            while let Some(res) = join_set.join_next().await {
                let (node_name, next_payload) = res.expect("task panicked")?;
                branch_results.push((node_name, next_payload));
            }

            // Merge payloads from all parallel branches
            let mut final_payload = current_snapshot.payload().clone();
            for (_, payload) in &branch_results {
                merge_json(&mut final_payload, payload);
            }

            // Determine next nodes
            let mut next_node_names = Vec::new();
            let mut escalate = false;

            for (node_name, payload) in &branch_results {
                let outgoing: Vec<_> = edges.iter().filter(|e| e.from() == *node_name).collect();

                if outgoing.is_empty() {
                    continue; // Terminal path for this branch
                }

                for edge in outgoing {
                    let next_node_name = edge.to().to_owned();

                    match self
                        .router
                        .evaluate_transition(node_name, &next_node_name, payload)
                    {
                        RouterAction::Escalate(target) => {
                            println!(
                                "Confidence Router: Escalating task for node '{target}' to frontier model."
                            );
                            escalate = true;
                        }
                        RouterAction::Continue => {}
                    }

                    next_node_names.push(next_node_name);
                }
            }

            if escalate && let Some(obj) = final_payload.as_object_mut() {
                obj.insert("escalation_required".to_string(), Value::Bool(true));
            }

            if next_node_names.is_empty() {
                // All branches reached terminal nodes
                let transition = Transition::new(current_node_names.join(", "), "END");

                current_snapshot = current_snapshot
                    .transition(transition, final_payload)
                    .map_err(|_| ExecutorError::InvalidTransition)?;

                let mut store = self
                    .state_store
                    .lock()
                    .map_err(|_| ExecutorError::PoisonedLock)?;
                store
                    .append(current_snapshot)
                    .map_err(ExecutorError::State)?;
                break;
            }

            let transition =
                Transition::new(current_node_names.join(", "), next_node_names.join(", "));

            current_snapshot = current_snapshot
                .transition(transition, final_payload)
                .map_err(|_| ExecutorError::InvalidTransition)?;

            // Persist state transition safely and atomically (synchronized fan-in)
            {
                let mut store = self
                    .state_store
                    .lock()
                    .map_err(|_| ExecutorError::PoisonedLock)?;
                store
                    .append(current_snapshot.clone())
                    .map_err(ExecutorError::State)?;
            }

            current_node_names = next_node_names;
        }

        Ok(execution_id)
    }
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("state storage error: {0}")]
    State(#[from] StateError),
    #[error("invalid transition produced during execution")]
    InvalidTransition,
    #[error("state store lock was poisoned")]
    PoisonedLock,
    #[error("supervisor interaction failed")]
    SupervisorFailed,
}
