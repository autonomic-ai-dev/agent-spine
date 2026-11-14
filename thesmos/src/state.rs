use std::collections::HashMap;

use rusqlite::{Connection, params};
use thiserror::Error;

use crate::{ExecutionId, StateSnapshot, WorkflowState};

/// In-memory state store, used by tests. For production use, prefer `SqliteStateStore`.
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

    fn list_executions(&self) -> Result<Vec<ExecutionId>, StateError> {
        Ok(self.snapshots.keys().copied().collect())
    }
}

/// A SQLite-backed state store for robust, atomic persistence.
pub struct SqliteStateStore {
    conn: Connection,
}

impl SqliteStateStore {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, StateError> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                snapshot_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(execution_id, sequence)
            )",
            [],
        )?;
        Ok(Self { conn })
    }
}

impl WorkflowState for SqliteStateStore {
    #[tracing::instrument(skip(self, snapshot), fields(execution_id = ?snapshot.execution_id(), seq = snapshot.sequence()))]
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), StateError> {
        let execution_id_str = execution_id_to_string(snapshot.execution_id());

        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM snapshots WHERE execution_id = ?1")?;
        let count_i64: i64 = stmt.query_row([&execution_id_str], |row| row.get(0))?;
        let expected_sequence = count_i64 as u64;

        if snapshot.sequence() != expected_sequence {
            return Err(StateError::InvalidSequence {
                expected: expected_sequence,
                actual: snapshot.sequence(),
            });
        }

        let json = serde_json::to_string(&snapshot).map_err(StateError::Serialization)?;
        self.conn.execute(
            "INSERT INTO snapshots (execution_id, sequence, snapshot_data) VALUES (?1, ?2, ?3)",
            params![execution_id_str, snapshot.sequence(), json],
        )?;

        Ok(())
    }

    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot> {
        let execution_id_str = execution_id_to_string(execution_id);
        let mut history = Vec::new();

        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT snapshot_data FROM snapshots WHERE execution_id = ?1 ORDER BY sequence ASC",
        ) {
            let snapshot_iter = stmt.query_map([&execution_id_str], |row| row.get::<_, String>(0));
            if let Ok(iter) = snapshot_iter {
                for json in iter.flatten() {
                    if let Ok(snapshot) = serde_json::from_str(&json) {
                        history.push(snapshot);
                    }
                }
            }
        }

        history
    }

    fn list_executions(&self) -> Result<Vec<ExecutionId>, StateError> {
        let mut ids = Vec::new();
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT execution_id FROM snapshots")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let id_str = row?;
            if let Ok(id) = execution_id_from_string(&id_str) {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum StateError {
    #[error("snapshot sequence mismatch: expected {expected}, received {actual}")]
    InvalidSequence { expected: u64, actual: u64 },
    #[error("snapshot history exceeded supported sequence range")]
    Overflow,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

fn execution_id_to_string(id: ExecutionId) -> String {
    serde_json::to_string(&id)
        .unwrap_or_else(|_| "unknown".to_string())
        .trim_matches('"')
        .to_string()
}

fn execution_id_from_string(s: &str) -> Result<ExecutionId, StateError> {
    serde_json::from_str::<ExecutionId>(&format!("\"{}\"", s)).map_err(StateError::Serialization)
}
