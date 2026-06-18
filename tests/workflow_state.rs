use agent_spine::{
    ExecutionId, StateSnapshot, Transition, WorkflowState, state::InMemoryStateStore,
};
use serde_json::json;

#[test]
fn state_transitions_are_immutable_and_replayable() {
    let execution_id = ExecutionId::new();
    let initial = StateSnapshot::initial(execution_id, json!({"task": "design engine"}));
    let transition = Transition::new("plan", "implement");

    let next = initial
        .transition(
            transition,
            json!({"task": "design engine", "approved": true}),
        )
        .expect("valid transition");

    assert_eq!(initial.sequence(), 0);
    assert_eq!(initial.payload(), &json!({"task": "design engine"}));
    assert_eq!(next.sequence(), 1);
    assert_eq!(next.parent_id(), Some(initial.id()));
    assert_eq!(next.payload()["approved"], true);
}

#[test]
fn state_store_preserves_snapshot_history() {
    let execution_id = ExecutionId::new();
    let snapshot = StateSnapshot::initial(execution_id, json!({"step": 0}));
    let mut store = InMemoryStateStore::default();

    store
        .append(snapshot.clone())
        .expect("append initial state");

    assert_eq!(store.history(execution_id), vec![snapshot]);
}

#[test]
fn empty_transition_nodes_are_rejected() {
    let snapshot = StateSnapshot::initial(ExecutionId::new(), json!({}));

    let error = snapshot
        .transition(Transition::new("", "implement"), json!({}))
        .expect_err("empty source node must fail");

    assert_eq!(error.to_string(), "transition node names must not be empty");
}

#[test]
fn state_store_rejects_out_of_order_snapshots() {
    let execution_id = ExecutionId::new();
    let initial = StateSnapshot::initial(execution_id, json!({"step": 0}));
    let next = initial
        .transition(Transition::new("one", "two"), json!({"step": 1}))
        .expect("valid transition");
    let mut store = InMemoryStateStore::default();

    let error = store
        .append(next)
        .expect_err("sequence one cannot be the first snapshot");

    assert_eq!(
        error.to_string(),
        "snapshot sequence mismatch: expected 0, received 1"
    );
}
