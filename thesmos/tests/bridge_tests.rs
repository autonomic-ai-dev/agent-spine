use std::sync::{Arc, Mutex};
use thesmos::executor::Executor;
use thesmos::state::InMemoryStateStore;
use thesmos::supervisor::Supervisor;
use thesmos::workflow::{NodeKind, WorkflowDefinition, WorkflowEdge, WorkflowNode};

#[test]
fn check_send_types() {
    let nodes = vec![WorkflowNode::new("start", NodeKind::Agent)];
    let edges = vec![WorkflowEdge::new("start", "start")];
    let def = WorkflowDefinition::new("test", 1, "start", nodes, edges);
    let validated = def.validate().unwrap();
    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();
    let _executor = Executor::<InMemoryStateStore>::new(validated, store, supervisor);
}
