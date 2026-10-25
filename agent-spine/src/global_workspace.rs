use crate::execution::ExecutionId;
use serde_json::Value;
use std::io;
use std::path::PathBuf;

use crate::{StateSnapshot, WorkflowState};

pub fn autonomic_root() -> PathBuf {
    std::env::var("AUTONOMIC_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".autonomic"))
}

fn home_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn spine_logs_dir() -> PathBuf {
    autonomic_root().join("logs").join("spine")
}

pub fn default_state_db() -> PathBuf {
    spine_logs_dir().join("state.db")
}

pub fn executions_dir() -> PathBuf {
    spine_logs_dir().join("executions")
}

pub fn spine_config_dir() -> PathBuf {
    autonomic_root().join("state").join("spine")
}

pub fn legacy_spine_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-spine")
}

pub fn ensure_dirs() -> io::Result<()> {
    std::fs::create_dir_all(spine_logs_dir())?;
    std::fs::create_dir_all(executions_dir())?;
    std::fs::create_dir_all(autonomic_root())?;
    Ok(())
}

/// Resolve CLI `--db` default
pub fn resolve_state_db(db: Option<PathBuf>) -> io::Result<PathBuf> {
    ensure_dirs().map_err(io::Error::other)?;
    Ok(db.unwrap_or_else(default_state_db))
}

/// Persist the full snapshot history as a JSON execution graph for observability.
pub fn export_execution_graph(
    store: &dyn WorkflowState,
    execution_id: ExecutionId,
    workflow_name: &str,
) -> io::Result<PathBuf> {
    ensure_dirs().map_err(io::Error::other)?;
    let history = store.history(execution_id);
    let path = executions_dir().join(format!("{execution_id}.json"));
    let payload = serde_json::json!({
        "execution_id": execution_id.to_string(),
        "workflow_name": workflow_name,
        "snapshot_count": history.len(),
        "snapshots": history,
    });
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&payload).map_err(io::Error::other)?,
    )?;
    Ok(path)
}

/// Write a lightweight DAG summary alongside the full graph export.
pub fn export_dag_summary(
    history: &[StateSnapshot],
    execution_id: ExecutionId,
) -> io::Result<PathBuf> {
    ensure_dirs().map_err(io::Error::other)?;
    let nodes: Vec<Value> = history
        .iter()
        .enumerate()
        .map(|(idx, snap)| {
            serde_json::json!({
                "sequence": idx,
                "transition": snap.transition_edge().map(|t| format!("{} -> {}", t.from(), t.to())),
                "payload_keys": snap.payload().as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()),
            })
        })
        .collect();
    let path = executions_dir().join(format!("{execution_id}.dag.json"));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "execution_id": execution_id.to_string(),
            "nodes": nodes,
        }))
        .map_err(io::Error::other)?,
    )?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_db_lives_under_autonomic_logs() {
        let db = default_state_db();
        assert!(db.to_string_lossy().contains(".autonomic"));
        assert!(db.to_string_lossy().contains("spine"));
    }
}
