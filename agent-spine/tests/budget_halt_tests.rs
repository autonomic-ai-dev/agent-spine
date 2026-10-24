use std::sync::{Arc, Mutex};

use agent_spine::budget_gate::BudgetGate;
use agent_spine::executor::{Executor, ExecutorError};
use agent_spine::state::InMemoryStateStore;
use agent_spine::supervisor::Supervisor;
use agent_spine::workflow::{NodeKind, WorkflowDefinition, WorkflowNode};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn http_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn heart_answering(response: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request).await;
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

fn agent_workflow() -> agent_spine::workflow::ValidatedWorkflow {
    let nodes = vec![WorkflowNode::new("start_agent", NodeKind::Agent)];
    WorkflowDefinition::new("budget_halt", 1, "start_agent", nodes, vec![])
        .validate()
        .expect("valid workflow")
}

#[tokio::test]
async fn workflow_halts_before_the_node_runs_when_heart_answers_500() {
    let heart = heart_answering(http_response(
        "500 Internal Server Error",
        r#"{"ok":false,"error":"db"}"#,
    ))
    .await;
    let supervisor = Supervisor::new();
    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let mut executor = Executor::new(agent_workflow(), store, supervisor.clone())
        .with_budget_gate(BudgetGate::with_heart_url(heart));

    let err = executor.run(json!({ "init": true })).await.unwrap_err();

    assert!(
        matches!(err, ExecutorError::BudgetUnavailable { .. }),
        "{err:?}"
    );
    assert!(supervisor.pending_tasks().is_empty());
}

#[tokio::test]
async fn workflow_dispatches_the_node_when_heart_approves() {
    let heart = heart_answering(http_response(
        "200 OK",
        r#"{"ok":true,"decision":{"allowed":true,"frozen":false,"reason":""}}"#,
    ))
    .await;
    let supervisor = Supervisor::new();
    let store = Arc::new(Mutex::new(InMemoryStateStore::default()));
    let mut executor = Executor::new(agent_workflow(), store, supervisor.clone())
        .with_budget_gate(BudgetGate::with_heart_url(heart));

    let run = tokio::spawn(async move { executor.run(json!({ "init": true })).await });

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while supervisor.pending_tasks() != vec!["start_agent".to_string()] {
        assert!(tokio::time::Instant::now() < deadline, "node was never dispatched");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    run.abort();
}
