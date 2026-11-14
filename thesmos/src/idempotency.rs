use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A record of an external effect that must not be duplicated on replay.
///
/// Each effect is identified by a unique `key` (e.g., `"api:post:order/123"`,
/// `"git:push:feature-x"`). When the same workflow execution is replayed,
/// idempotency keys that are already consumed are skipped — the recorded
/// `result` is returned directly without re-executing the side effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    /// Unique key for this effect.
    pub key: String,
    /// The execution ID that first consumed this key.
    pub execution_id: String,
    /// JSON-encoded result payload from the effect.
    pub result: String,
    /// Human-readable description of the effect.
    pub description: String,
    /// Unix timestamp (ms) when the record was created.
    pub created_at_ms: u64,
}

/// Tracks consumed idempotency keys to prevent duplicate side effects.
pub trait IdempotencyStore: Send {
    /// Check if a key has already been consumed.
    fn is_consumed(&self, key: &str) -> Result<bool, IdempotencyError>;
    /// Mark a key as consumed with its result.
    fn mark_consumed(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyError>;
    /// Retrieve a consumed record by key.
    fn get_result(&self, key: &str) -> Result<Option<IdempotencyRecord>, IdempotencyError>;
    /// List all consumed keys for an execution.
    fn list_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<IdempotencyRecord>, IdempotencyError>;
}

/// In-memory idempotency store (dev/testing).
#[derive(Default)]
pub struct InMemoryIdempotencyStore {
    records: Mutex<Vec<IdempotencyRecord>>,
}

impl IdempotencyStore for InMemoryIdempotencyStore {
    fn is_consumed(&self, key: &str) -> Result<bool, IdempotencyError> {
        let guard = self
            .records
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        Ok(guard.iter().any(|r| r.key == key))
    }

    fn mark_consumed(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyError> {
        let mut guard = self
            .records
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        guard.push(record.clone());
        Ok(())
    }

    fn get_result(&self, key: &str) -> Result<Option<IdempotencyRecord>, IdempotencyError> {
        let guard = self
            .records
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        Ok(guard.iter().find(|r| r.key == key).cloned())
    }

    fn list_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<IdempotencyRecord>, IdempotencyError> {
        let guard = self
            .records
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        Ok(guard
            .iter()
            .filter(|r| r.execution_id == execution_id)
            .cloned()
            .collect())
    }
}

/// Persistent SQLite-based idempotency store.
pub struct SqliteIdempotencyStore {
    conn: Mutex<Connection>,
}

impl SqliteIdempotencyStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, IdempotencyError> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS idempotency_keys (
                key TEXT PRIMARY KEY,
                execution_id TEXT NOT NULL,
                result TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                created_at_ms INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_idempotency_execution_id
             ON idempotency_keys (execution_id)",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl IdempotencyStore for SqliteIdempotencyStore {
    fn is_consumed(&self, key: &str) -> Result<bool, IdempotencyError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        let mut stmt = conn.prepare("SELECT 1 FROM idempotency_keys WHERE key = ?1")?;
        Ok(stmt.exists(rusqlite::params![key])?)
    }

    fn mark_consumed(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        conn.execute(
            "INSERT OR IGNORE INTO idempotency_keys (key, execution_id, result, description, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                record.key,
                record.execution_id,
                record.result,
                record.description,
                record.created_at_ms,
            ],
        )?;
        Ok(())
    }

    fn get_result(&self, key: &str) -> Result<Option<IdempotencyRecord>, IdempotencyError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        let mut stmt = conn.prepare(
            "SELECT key, execution_id, result, description, created_at_ms
             FROM idempotency_keys WHERE key = ?1",
        )?;
        let mut rows = stmt.query(rusqlite::params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(IdempotencyRecord {
                key: row.get(0)?,
                execution_id: row.get(1)?,
                result: row.get(2)?,
                description: row.get(3)?,
                created_at_ms: row.get(4)?,
            })),
            None => Ok(None),
        }
    }

    fn list_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<IdempotencyRecord>, IdempotencyError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| IdempotencyError::PoisonedLock)?;
        let mut stmt = conn.prepare(
            "SELECT key, execution_id, result, description, created_at_ms
             FROM idempotency_keys WHERE execution_id = ?1
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![execution_id], |row| {
            Ok(IdempotencyRecord {
                key: row.get(0)?,
                execution_id: row.get(1)?,
                result: row.get(2)?,
                description: row.get(3)?,
                created_at_ms: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IdempotencyError::Sqlite)
    }
}

#[derive(Debug, Error)]
pub enum IdempotencyError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("state store lock was poisoned")]
    PoisonedLock,
}

/// Check if an effect key has been consumed; return the cached result if so.
/// This is the primary API called by the executor before executing a side effect.
pub fn check_idempotency(
    store: &dyn IdempotencyStore,
    key: &str,
) -> Result<Option<String>, IdempotencyError> {
    if store.is_consumed(key)? {
        let record = store.get_result(key)?;
        Ok(record.map(|r| r.result))
    } else {
        Ok(None)
    }
}
