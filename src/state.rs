use std::collections::HashMap;

use thiserror::Error;

use crate::{ExecutionId, StateSnapshot, WorkflowState};

/// Append-only state adapter used by tests and early engine development.
#[derive(Default)]
pub struct InMemoryStateStore {
    snapshots: HashMap<ExecutionId, Vec<StateSnapshot>>,
}

impl WorkflowState for InMemoryStateStore {
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), StateError> {
        let history = self.snapshots.entry(snapshot.execution_id()).or_default();
        let expected_sequence = u64::try_from(history.len()).map_err(|_| StateError::Overflow)?;

        if snapshot.sequence() != expected_sequence {
            return Err(StateError::InvalidSequence {
                expected: expected_sequence,
                actual: snapshot.sequence(),
            });
        }

        history.push(snapshot);
        Ok(())
    }

    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot> {
        self.snapshots
            .get(&execution_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("snapshot sequence mismatch: expected {expected}, received {actual}")]
    InvalidSequence { expected: u64, actual: u64 },
    #[error("snapshot history exceeded supported sequence range")]
    Overflow,
}
