use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use rusqlite::{Connection, params};
use thiserror::Error;

#[cfg(any(feature = "postgres", feature = "redis"))]
use std::future::Future;
#[cfg(feature = "redis")]
use std::sync::Mutex;

use crate::{ExecutionId, StateSnapshot, WorkflowState};

/// Append-only state adapter used by tests and early engine development.
///
/// For production use, prefer `SqliteStateStore`, `PostgresStateStore` (with `postgres` feature),
/// or `RedisStateStore` (with `redis` feature).
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

/// A file-backed append-only state store using JSONL.
/// Useful for time-travel debugging and persistent executions.
pub struct FileStateStore {
    base_dir: PathBuf,
}

impl FileStateStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self, StateError> {
        let base_dir = base_dir.into();
        fs::create_dir_all(&base_dir).map_err(StateError::Io)?;
        Ok(Self { base_dir })
    }

    fn file_path(&self, execution_id: ExecutionId) -> PathBuf {
        let file_name = format!(
            "{}.jsonl",
            serde_json::to_string(&execution_id)
                .unwrap_or_else(|_| "unknown".to_string())
                .trim_matches('"')
        );
        self.base_dir.join(file_name)
    }
}

impl WorkflowState for FileStateStore {
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), StateError> {
        let path = self.file_path(snapshot.execution_id());

        let history = self.history(snapshot.execution_id());
        let expected_sequence = u64::try_from(history.len()).map_err(|_| StateError::Overflow)?;

        if snapshot.sequence() != expected_sequence {
            return Err(StateError::InvalidSequence {
                expected: expected_sequence,
                actual: snapshot.sequence(),
            });
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(StateError::Io)?;

        let json = serde_json::to_string(&snapshot).map_err(StateError::Serialization)?;
        writeln!(file, "{json}").map_err(StateError::Io)?;

        Ok(())
    }

    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot> {
        let path = self.file_path(execution_id);
        let mut history = Vec::new();

        if let Ok(file) = fs::File::open(&path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(snapshot) = serde_json::from_str(&line) {
                    history.push(snapshot);
                }
            }
        }

        history
    }

    fn list_executions(&self) -> Result<Vec<ExecutionId>, StateError> {
        let mut ids = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("jsonl")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    && let Ok(id) = serde_json::from_str::<ExecutionId>(&format!("\"{}\"", stem))
                {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
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

// ── Postgres ──────────────────────────────────────────────────────────────

/// A PostgreSQL-backed state store using JSONB columns.
///
/// Requires the `postgres` feature. Create with a connection URL:
/// ```ignore
/// let store = PostgresStateStore::new("postgres://user:pass@localhost/db").await?;
/// ```
#[cfg(feature = "postgres")]
pub struct PostgresStateStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresStateStore {
    pub async fn new(connection_url: &str) -> Result<Self, StateError> {
        let pool = sqlx::PgPool::connect(connection_url)
            .await
            .map_err(to_io_error)?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id SERIAL PRIMARY KEY,
                execution_id VARCHAR(255) NOT NULL,
                sequence BIGINT NOT NULL,
                snapshot_data JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(execution_id, sequence)
            )",
        )
        .execute(&pool)
        .await
        .map_err(to_io_error)?;

        // Index for execution lookups
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_snapshots_execution_id
             ON snapshots (execution_id, sequence)",
        )
        .execute(&pool)
        .await
        .map_err(to_io_error)?;

        Ok(Self { pool })
    }
}

#[cfg(feature = "postgres")]
impl WorkflowState for PostgresStateStore {
    #[tracing::instrument(skip(self, snapshot), fields(execution_id = ?snapshot.execution_id(), seq = snapshot.sequence()))]
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), StateError> {
        let execution_id_str = execution_id_to_string(snapshot.execution_id());

        let expected_sequence = block_on(async {
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM snapshots WHERE execution_id = $1")
                    .bind(&execution_id_str)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(to_io_error)?;
            Ok::<u64, StateError>(count.0 as u64)
        })?;

        if snapshot.sequence() != expected_sequence {
            return Err(StateError::InvalidSequence {
                expected: expected_sequence,
                actual: snapshot.sequence(),
            });
        }

        let json = serde_json::to_value(&snapshot).map_err(StateError::Serialization)?;

        block_on(async {
            sqlx::query(
                "INSERT INTO snapshots (execution_id, sequence, snapshot_data)
                 VALUES ($1, $2, $3)",
            )
            .bind(&execution_id_str)
            .bind(snapshot.sequence() as i64)
            .bind(&json)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(to_io_error)
        })
    }

    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot> {
        let execution_id_str = execution_id_to_string(execution_id);

        let rows = block_on::<Result<Vec<(serde_json::Value,)>, StateError>>(async {
            sqlx::query_as(
                "SELECT snapshot_data FROM snapshots
                 WHERE execution_id = $1 ORDER BY sequence ASC",
            )
            .bind(&execution_id_str)
            .fetch_all(&self.pool)
            .await
            .map_err(to_io_error)
        })
        .unwrap_or_default();

        rows.into_iter()
            .filter_map(|(json,)| serde_json::from_value(json).ok())
            .collect()
    }

    fn list_executions(&self) -> Result<Vec<ExecutionId>, StateError> {
        let rows: Vec<(String,)> = block_on(async {
            sqlx::query_as("SELECT DISTINCT execution_id FROM snapshots")
                .fetch_all(&self.pool)
                .await
                .map_err(to_io_error)
        })?;

        rows.iter()
            .filter_map(|(id_str,)| execution_id_from_string(id_str).ok())
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
}

// ── Redis ─────────────────────────────────────────────────────────────────

/// An ephemeral Redis-backed state store for fast status checks.
///
/// Requires the `redis` feature. Snapshots are stored as JSON strings keyed by
/// `agent-spine:{execution_id}:{sequence}` with a sorted set for ordered history.
///
/// ⚠ This store is best for live dashboards and status checks. For durable
/// persistence, use `PostgresStateStore` or `SqliteStateStore`.
#[cfg(feature = "redis")]
pub struct RedisStateStore {
    conn: Mutex<redis::aio::MultiplexedConnection>,
}

#[cfg(feature = "redis")]
impl RedisStateStore {
    pub async fn new(connection_url: &str) -> Result<Self, StateError> {
        let client = redis::Client::open(connection_url)
            .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let conn = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn snapshot_key(execution_id: &str, sequence: u64) -> String {
        format!("agent-spine:snap:{execution_id}:{sequence}")
    }

    fn history_key(execution_id: &str) -> String {
        format!("agent-spine:history:{execution_id}")
    }
}

#[cfg(feature = "redis")]
impl WorkflowState for RedisStateStore {
    #[tracing::instrument(skip(self, snapshot), fields(execution_id = ?snapshot.execution_id(), seq = snapshot.sequence()))]
    fn append(&mut self, snapshot: StateSnapshot) -> Result<(), StateError> {
        let execution_id_str = execution_id_to_string(snapshot.execution_id());
        let mut conn = self.conn.lock().map_err(|_| {
            StateError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "poisoned lock",
            ))
        })?;

        let expected_sequence: u64 = block_on(async {
            let count: u64 = redis::cmd("ZCARD")
                .arg(&[Self::history_key(&execution_id_str)])
                .query_async(&mut *conn)
                .await
                .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            Ok::<u64, StateError>(count)
        })?;

        if snapshot.sequence() != expected_sequence {
            return Err(StateError::InvalidSequence {
                expected: expected_sequence,
                actual: snapshot.sequence(),
            });
        }

        let json = serde_json::to_string(&snapshot).map_err(StateError::Serialization)?;
        let snap_key = Self::snapshot_key(&execution_id_str, snapshot.sequence());
        let history_key = Self::history_key(&execution_id_str);

        block_on(async {
            let mut pipe = redis::pipe();
            pipe.set(&snap_key, &json)
                .zadd(&history_key, snapshot.sequence(), &snap_key)
                .ignore()
                .query_async(&mut *conn)
                .await
                .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
        })
    }

    fn history(&self, execution_id: ExecutionId) -> Vec<StateSnapshot> {
        let execution_id_str = execution_id_to_string(execution_id);
        let history_key = Self::history_key(&execution_id_str);

        let mut conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let keys: Vec<String> = block_on(async {
            redis::cmd("ZRANGE")
                .arg(&[&history_key, "0", "-1"])
                .query_async(&mut *conn)
                .await
                .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
        })
        .unwrap_or_default();

        if keys.is_empty() {
            return Vec::new();
        }

        let values: Vec<String> = block_on(async {
            redis::cmd("MGET")
                .arg(&keys)
                .query_async(&mut *conn)
                .await
                .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
        })
        .unwrap_or_default();

        values
            .iter()
            .filter_map(|json| serde_json::from_str(json).ok())
            .collect()
    }

    fn list_executions(&self) -> Result<Vec<ExecutionId>, StateError> {
        let mut conn = self.conn.lock().map_err(|_| {
            StateError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "poisoned lock",
            ))
        })?;

        let keys: Vec<String> = block_on(async {
            redis::cmd("KEYS")
                .arg("agent-spine:snap:*")
                .query_async(&mut *conn)
                .await
                .map_err(|e| StateError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
        })?;

        let mut ids = Vec::new();
        for key in keys {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 3 {
                if let Ok(id) = execution_id_from_string(parts[2]) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
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
    #[error("store not available: the '{0}' feature is not enabled")]
    FeatureNotEnabled(&'static str),
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

/// Run an async operation synchronously using `block_in_place` + `block_on`.
/// This is required because `WorkflowState` methods are synchronous but some
/// backends (Postgres, Redis) are inherently async. Only works in a
/// multi-threaded tokio runtime.
#[cfg(any(feature = "postgres", feature = "redis"))]
fn block_on<T>(fut: impl Future<Output = T>) -> T {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut))
}

#[cfg(any(feature = "postgres", feature = "redis"))]
fn to_io_error(e: impl std::fmt::Display) -> StateError {
    StateError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.to_string(),
    ))
}

#[cfg(feature = "postgres")]
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R;
}

#[cfg(feature = "postgres")]
impl<T> Pipe for T {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
