# 4. Concurrency and state

## What we built

agent-spine supports parallel execution through **fan-out/fan-in** edges. When a node has multiple outgoing edges, all downstream nodes execute concurrently. A JoinSet node merges the results of all inbound branches before proceeding.

### State stores

agent-spine supports three state store backends:

| Store | Format | Use case |
|-------|--------|----------|
| `InMemory` | Vec<Snapshot> in RAM | Development, testing, ephemeral workflows |
| `JSONL` | Append-only JSON lines file | Simple file-based persistence, grep-able |
| `SQLite` | Structured database | Production, multi-workflow storage, queryable |

Each snapshot contains:

```rust
struct Snapshot {
    execution_id: String,
    node_id: String,
    node_kind: NodeKind,
    parent_snapshot_id: Option<String>,
    sequence: u64,
    timestamp: i64,
    status: SnapshotStatus,  // Pending | Running | Completed | Failed | Paused
    input: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
}
```

### Concurrency model

Fan-out uses tokio `JoinSet`:

```rust
let mut join_set = JoinSet::new();
for child in children {
    join_set.spawn(execute_node(child, input.clone()));
}
while let Some(result) = join_set.join_next().await {
    results.push(result??);
}
```

This means branches run on the same async runtime, sharing the thread pool. The max concurrency is bounded by the runtime's thread count (default: number of CPU cores).

## Why this way

- **Immutability prevents race conditions.** Since snapshots are append-only, concurrent branch executions cannot conflict. Each snapshot stores its own input/output without shared mutable state.
- **JoinSet was chosen over channels or actors** because it maps naturally to fan-out/fan-in: spawn N tasks, collect all results. No channel setup, no actor supervision needed.
- **Multiple store backends** let developers choose the right durability/queryability tradeoff for their environment.

## Alternatives considered

| Option | Why rejected |
|--------|-------------|
| **Channels (mpsc/broadcast)** | More boilerplate for the common fan-out pattern; JoinSet is cleaner |
| **Actor framework (Actix)** | Heavy dependency for a simple pattern; tokio JoinSet is sufficient |
| **PostgreSQL store** | Would require an external database; SQLite is zero-config |

## Trade-offs

- **InMemory snapshots are lost on restart.** Fine for dev, but production should use SQLite or JSONL.
- **JoinSet uses tokio's work-stealing thread pool.** CPU-bound branches can starve I/O-bound ones if they don't yield. Consider `tokio::task::spawn_blocking` for CPU-heavy work.
- **JSONL store is append-only — no compaction.** Over time, the file grows unbounded. Not suitable for long-running production systems without a rotation strategy.

## For senior engineers / PEs

- **The state store trait is the concurrency bottleneck.** All store implementations use `Arc<Mutex<dyn StateStore>>`, meaning concurrent reads are serialized. This is acceptable for local workflows (<100 ops/s) but would need sharding for high-throughput deployments.
- **Snapshot sequence numbers are monotonic per execution.** They serve as a logical clock: if snapshot N has parent N-1, the chain is complete. Gaps indicate lost snapshots.
- **Replay** creates a *new* execution branch with a new execution ID. The old branch is preserved. This means replay is never destructive — you can always compare the original and replayed paths.
