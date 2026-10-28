use serde_json::json;
use std::sync::{Arc, Mutex};

use agent_spine::WorkflowState;
use agent_spine::executor::Executor;
use agent_spine::router::{ConfidenceRouter, RouterAction};
use agent_spine::state::InMemoryStateStore;
use agent_spine::supervisor::Supervisor;
use agent_spine::workflow::{NodeKind, WorkflowDefinition, WorkflowEdge, WorkflowNode};

async fn wait_for_pending(supervisor: &Supervisor, expected: &[&str]) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut want: Vec<&str> = expected.to_vec();
    want.sort();
    loop {
        let mut pending = supervisor.pending_tasks();
        pending.sort();
        if pending == want {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pending tasks {pending:?} never reached {want:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn test_executor_linear_run() {
    let nodes = vec![
        WorkflowNode::new("start_agent", NodeKind::Agent),
        WorkflowNode::new("end_agent", NodeKind::Agent),
    ];
    let edges = vec![WorkflowEdge::new("start_agent", "end_agent")];

    let def = WorkflowDefinition::new("test_linear", 1, "start_agent", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start_agent"]).await;

    supervisor
        .resume("start_agent", json!({ "step": 1 }))
        .unwrap();

    wait_for_pending(&supervisor, &["end_agent"]).await;

    supervisor
        .resume("end_agent", json!({ "step": 2 }))
        .unwrap();

    let execution_id = exec_task.await.unwrap().unwrap();

    let history = store.lock().unwrap().history(execution_id);
    assert_eq!(history.len(), 3);
}

#[test]
fn test_confidence_router_escalation() {
    let mut router = ConfidenceRouter::new(3);

    assert_eq!(
        router.evaluate_transition("Agent", "Verify", &json!({ "success": false })),
        RouterAction::Continue
    );
    assert_eq!(
        router.evaluate_transition("Agent", "Verify", &json!({ "success": false })),
        RouterAction::Continue
    );
    assert_eq!(
        router.evaluate_transition("Agent", "Verify", &json!({ "success": false })),
        RouterAction::Escalate("Verify".to_string())
    );
}

#[tokio::test]
async fn test_executor_parallel_fan_out() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::new("branch_b", NodeKind::Agent),
        WorkflowNode::new("branch_c", NodeKind::Agent),
        WorkflowNode::new("end", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "branch_b"),
        WorkflowEdge::new("start", "branch_c"),
        WorkflowEdge::new("branch_b", "end"),
        WorkflowEdge::new("branch_c", "end"),
    ];

    let def = WorkflowDefinition::new("test_parallel", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "start_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["branch_b", "branch_c"]).await;

    supervisor
        .resume("branch_b", json!({ "b_done": true }))
        .unwrap();
    supervisor
        .resume("branch_c", json!({ "c_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor.resume("end", json!({ "final": true })).unwrap();

    let execution_id = exec_task.await.unwrap().unwrap();

    let history = store.lock().unwrap().history(execution_id);
    assert_eq!(history.len(), 4);

    let fan_in_payload = history[2].payload();
    assert_eq!(fan_in_payload["b_done"], true);
    assert_eq!(fan_in_payload["c_done"], true);
    assert_eq!(fan_in_payload["start_done"], true);

    let final_payload = history[3].payload();
    assert_eq!(final_payload["final"], true);
    assert_eq!(final_payload["b_done"], true);
    assert_eq!(final_payload["c_done"], true);
}

#[tokio::test]
async fn test_approval_gate_accepts() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::approval_gate("gate"),
        WorkflowNode::new("end", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "gate"),
        WorkflowEdge::new("gate", "end"),
    ];

    let def = WorkflowDefinition::new("test_approval_accepts", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "start_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["gate"]).await;

    supervisor
        .resume("gate", json!({ "approved": true, "start_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor.resume("end", json!({ "final": true })).unwrap();

    let _ = exec_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_approval_gate_rejects() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::approval_gate("gate"),
        WorkflowNode::new("end", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "gate"),
        WorkflowEdge::new("gate", "end"),
    ];

    let def = WorkflowDefinition::new("test_approval_rejects", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "start_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["gate"]).await;

    supervisor
        .resume("gate", json!({ "rejected": true }))
        .unwrap();

    let err = exec_task.await.unwrap().unwrap_err();
    assert_eq!(err.to_string(), "execution rejected at approval gate");
}

#[tokio::test]
async fn test_supervisor_timeout_and_retry() {
    let nodes = vec![WorkflowNode::new("start", NodeKind::Agent)];
    let edges = vec![];

    let def = WorkflowDefinition::new("test_timeout", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));

    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;

    exec_task.abort();
}

#[tokio::test]
async fn test_fork_join_barrier() {
    let nodes = vec![
        WorkflowNode::fork("fork"),
        WorkflowNode::agent("branch_a"),
        WorkflowNode::agent("branch_b"),
        WorkflowNode::join("join"),
        WorkflowNode::agent("end"),
    ];
    let edges = vec![
        WorkflowEdge::new("fork", "branch_a"),
        WorkflowEdge::new("fork", "branch_b"),
        WorkflowEdge::new("branch_a", "join"),
        WorkflowEdge::new("branch_b", "join"),
        WorkflowEdge::new("join", "end"),
    ];

    let def = WorkflowDefinition::new("test_fork_join", 1, "fork", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["branch_a", "branch_b"]).await;

    supervisor
        .resume("branch_a", json!({ "a_done": true }))
        .unwrap();
    supervisor
        .resume("branch_b", json!({ "b_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor.resume("end", json!({ "final": true })).unwrap();

    let _ = exec_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_conditional_edge_skips_branch() {
    let nodes = vec![
        WorkflowNode::router("router"),
        WorkflowNode::agent("frontend"),
        WorkflowNode::agent("backend"),
        WorkflowNode::agent("end"),
    ];
    let edges = vec![
        WorkflowEdge::conditional("router", "frontend", r#"state.task_type == "frontend""#),
        WorkflowEdge::conditional("router", "backend", r#"state.task_type == "backend""#),
        WorkflowEdge::new("frontend", "end"),
        WorkflowEdge::new("backend", "end"),
    ];

    let def = WorkflowDefinition::new("test_conditional", 1, "router", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "state": { "task_type": "frontend" } }))
            .await
    });

    wait_for_pending(&supervisor, &["router"]).await;
    supervisor
        .resume("router", json!({ "state": { "task_type": "frontend" } }))
        .unwrap();

    wait_for_pending(&supervisor, &["frontend"]).await;
    supervisor
        .resume("frontend", json!({ "fe_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor.resume("end", json!({ "final": true })).unwrap();

    let _ = exec_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_fork_join_with_multi_level_branches() {
    let nodes = vec![
        WorkflowNode::fork("fork"),
        WorkflowNode::agent("long_branch_a"),
        WorkflowNode::agent("long_branch_b"),
        WorkflowNode::agent("long_branch_c"),
        WorkflowNode::join("join"),
        WorkflowNode::agent("end"),
    ];
    let edges = vec![
        WorkflowEdge::new("fork", "long_branch_a"),
        WorkflowEdge::new("fork", "long_branch_b"),
        WorkflowEdge::new("long_branch_a", "long_branch_c"),
        WorkflowEdge::new("long_branch_b", "join"),
        WorkflowEdge::new("long_branch_c", "join"),
        WorkflowEdge::new("join", "end"),
    ];

    let def = WorkflowDefinition::new("test_multi_level_fork_join", 1, "fork", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();

    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["long_branch_a", "long_branch_b"]).await;

    supervisor
        .resume("long_branch_a", json!({ "a_done": true }))
        .unwrap();
    supervisor
        .resume("long_branch_b", json!({ "b_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["long_branch_c"]).await;

    supervisor
        .resume("long_branch_c", json!({ "c_done": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor.resume("end", json!({ "final": true })).unwrap();

    let _ = exec_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_verify_node_active_command_execution() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::verify("run_check").with_command("echo spine_verified_ok"),
        WorkflowNode::new("finish", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "run_check"),
        WorkflowEdge::new("run_check", "finish"),
    ];

    let def = WorkflowDefinition::new("test_verify_active", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();
    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "init": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "ready": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["finish"]).await;
    supervisor
        .resume("finish", json!({ "done": true }))
        .unwrap();

    let res = exec_task.await.unwrap();
    assert!(res.is_ok());

    let store_guard = store.lock().unwrap();
    let history = store_guard.history(res.unwrap());
    let check_snap = history
        .iter()
        .find(|s| s.transition_edge().map(|t| t.from()) == Some("run_check"))
        .unwrap();
    assert_eq!(check_snap.payload()["state"]["verified"], true);
    assert_eq!(check_snap.payload()["state"]["exit_code"], 0);
    assert!(
        check_snap.payload()["state"]["stdout"]
            .as_str()
            .unwrap()
            .contains("spine_verified_ok")
    );
}

#[tokio::test]
async fn test_verify_node_failing_command_with_continue_on_failure() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::verify("failing_check").with_command("sh -c 'exit 42'"),
        WorkflowNode::new("finish", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "failing_check"),
        WorkflowEdge::new("failing_check", "finish"),
    ];

    let def = WorkflowDefinition::new("test_verify_continue", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();
    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "continue_on_failure": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "continue_on_failure": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["finish"]).await;
    supervisor
        .resume("finish", json!({ "done": true }))
        .unwrap();

    let res = exec_task.await.unwrap();
    assert!(res.is_ok());

    let store_guard = store.lock().unwrap();
    let history = store_guard.history(res.unwrap());
    let check_snap = history
        .iter()
        .find(|s| s.transition_edge().map(|t| t.from()) == Some("failing_check"))
        .unwrap();
    assert_eq!(check_snap.payload()["state"]["verified"], false);
    assert_eq!(check_snap.payload()["state"]["exit_code"], 42);
}

#[tokio::test]
async fn test_speculative_fork_node_execution() {
    let nodes = vec![
        WorkflowNode::new("start", NodeKind::Agent),
        WorkflowNode::speculative_fork("spec_fork"),
        WorkflowNode::new("end", NodeKind::Agent),
    ];
    let edges = vec![
        WorkflowEdge::new("start", "spec_fork"),
        WorkflowEdge::new("spec_fork", "end"),
    ];

    let def = WorkflowDefinition::new("test_speculative", 1, "start", nodes, edges);
    let validated = def.validate().expect("valid workflow");

    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let supervisor = Supervisor::new();
    let executor = Executor::new(validated, Arc::clone(&store), supervisor.clone());

    let exec_task = tokio::spawn(async move {
        let mut exec = executor;
        exec.run(json!({ "speculative": true })).await
    });

    wait_for_pending(&supervisor, &["start"]).await;
    supervisor
        .resume("start", json!({ "prepared": true }))
        .unwrap();

    wait_for_pending(&supervisor, &["end"]).await;
    supervisor
        .resume("end", json!({ "finished": true }))
        .unwrap();

    let res = exec_task.await.unwrap();
    assert!(res.is_ok());
}
