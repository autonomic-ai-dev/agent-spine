//! Core contracts for the Agent Spine workflow engine.

pub mod state;

mod execution;
mod snapshot;
mod transition;

pub use execution::ExecutionId;
pub use snapshot::StateSnapshot;
pub use transition::Transition;

/// Read and append immutable workflow snapshots.
pub trait WorkflowState {
    /// Persist a snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot violates append-only ordering or
    /// the store cannot represent its sequence.
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), state::StateError>;

    /// Return the ordered snapshot history for an execution.
    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot>;
}
