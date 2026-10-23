use serde_json::json;

use agent_spine::cancellation::CancelToken;
use agent_spine::idempotency::{
    IdempotencyRecord, IdempotencyStore, InMemoryIdempotencyStore, SqliteIdempotencyStore,
    check_idempotency,
};
use agent_spine::supervisor::{Supervisor, SupervisorError};

fn record(key: &str, execution_id: &str, result: &str) -> IdempotencyRecord {
    IdempotencyRecord {
        key: key.to_owned(),
        execution_id: execution_id.to_owned(),
        result: result.to_owned(),
        description: format!("effect {key}"),
        created_at_ms: 1,
    }
}

#[test]
fn unseen_key_is_not_consumed() {
    let store = InMemoryIdempotencyStore::default();

    assert_eq!(check_idempotency(&store, "api:post:order/1").unwrap(), None);
}

#[test]
fn consumed_key_returns_its_recorded_result() {
    let store = InMemoryIdempotencyStore::default();
    store
        .mark_consumed(&record("api:post:order/1", "exec-a", r#"{"status":201}"#))
        .unwrap();

    assert_eq!(
        check_idempotency(&store, "api:post:order/1").unwrap(),
        Some(r#"{"status":201}"#.to_owned())
    );
}

#[test]
fn list_for_execution_returns_only_that_executions_records() {
    let store = InMemoryIdempotencyStore::default();
    store.mark_consumed(&record("k1", "exec-a", "1")).unwrap();
    store.mark_consumed(&record("k2", "exec-b", "2")).unwrap();
    store.mark_consumed(&record("k3", "exec-a", "3")).unwrap();

    let mut keys: Vec<String> = store
        .list_for_execution("exec-a")
        .unwrap()
        .into_iter()
        .map(|r| r.key)
        .collect();
    keys.sort();

    assert_eq!(keys, vec!["k1", "k3"]);
}

#[test]
fn sqlite_store_keeps_consumed_keys_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idempotency.db");

    {
        let store = SqliteIdempotencyStore::new(&path).unwrap();
        store
            .mark_consumed(&record("git:push:feature-x", "exec-a", "pushed"))
            .unwrap();
    }

    let reopened = SqliteIdempotencyStore::new(&path).unwrap();
    assert_eq!(
        check_idempotency(&reopened, "git:push:feature-x").unwrap(),
        Some("pushed".to_owned())
    );
}

#[test]
fn cancel_token_starts_clear_and_latches_after_cancel() {
    let token = CancelToken::new();
    assert!(!token.is_cancelled());

    token.cancel();

    assert!(token.is_cancelled());
    assert!(token.clone().is_cancelled());
}

#[tokio::test]
async fn cancel_token_notifies_watchers() {
    let token = CancelToken::new();
    let mut rx = token.watch();
    assert!(!*rx.borrow());

    token.cancel();

    rx.changed().await.unwrap();
    assert!(*rx.borrow());
}

#[tokio::test]
async fn delegate_returns_the_payload_resumed_for_its_node() {
    let supervisor = Supervisor::new();
    let waiter = supervisor.clone();

    let task = tokio::spawn(async move {
        waiter
            .delegate(
                "review".to_owned(),
                "agent".to_owned(),
                None,
                "wf".to_owned(),
                json!({ "draft": true }),
                None,
            )
            .await
    });

    while supervisor.pending_tasks() != vec!["review"] {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    supervisor
        .resume("review", json!({ "approved": true }))
        .unwrap();

    assert_eq!(task.await.unwrap().unwrap(), json!({ "approved": true }));
    assert!(supervisor.pending_tasks().is_empty());
}

#[tokio::test]
async fn second_delegate_for_a_pending_node_is_rejected() {
    let supervisor = Supervisor::new();
    let first = supervisor.clone();

    let pending = tokio::spawn(async move {
        first
            .delegate(
                "review".to_owned(),
                "agent".to_owned(),
                None,
                "wf".to_owned(),
                json!({}),
                None,
            )
            .await
    });

    while supervisor.pending_tasks().is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let duplicate = supervisor
        .delegate(
            "review".to_owned(),
            "agent".to_owned(),
            None,
            "wf".to_owned(),
            json!({}),
            None,
        )
        .await;
    assert!(matches!(
        duplicate,
        Err(SupervisorError::AlreadyPending(ref node)) if node == "review"
    ));

    supervisor.resume("review", json!({})).unwrap();
    pending.await.unwrap().unwrap();
}

#[test]
fn resume_of_a_node_that_is_not_pending_fails() {
    let supervisor = Supervisor::new();

    assert!(supervisor.resume("missing", json!({})).is_err());
}
