# 2. Workflow execution model

## What we built

agent-spine workflows are defined in YAML with a versioned schema. Each workflow has a set of nodes (typed operations) and edges (control flow transitions). The executor walks the graph, resolving each node according to its type.

### Node types

| Type | Behavior |
|------|----------|
| `Agent` | Delegates to LocalAgent (in-process) or external agent via gRPC |
| `Checkpoint` | Records a snapshot without executing work |
| `Verify` | Runs a validation predicate; blocks downstream if it fails |
| `ApprovalGate` | Pauses execution; waits for external HTTP signal to approve/reject |

### Edge types

| Type | Behavior |
|------|----------|
| Sequential | Execute node B after node A completes |
| Fan-out | Execute multiple downstream nodes concurrently |
| Fan-in (JoinSet) | Wait for all inbound branches to complete before proceeding |

### Retry policy

Each node can specify an optional retry policy:

```yaml
retry:
  max_attempts: 3
  initial_delay_ms: 1000
  backoff_factor: 2.0  # exponential
```

If all attempts fail, the node is marked failed and the workflow either stops or escalates to a ConfidenceRouter (threshold-based routing to a fallback path).

## Why this way

The YAML DAG model was chosen over programmatic APIs (like Temporal or LangGraph) for three reasons:

1. **Version control.** YAML files are diffable, reviewable in PRs, and don't require recompilation.
2. **Tool-agnostic.** Any agent harness can generate a YAML workflow — it doesn't need to be a Rust program.
3. **Validation before execution.** The entire workflow can be validated (`agent-spine validate`) without running it, catching errors early.

## Alternatives considered

| Option | Why rejected |
|--------|-------------|
| **JSON Schema + JSON** | JSON lacks comments and is less readable for complex nested structures |
| **Protobuf / Thrift** | Binary formats are not hand-editable or PR-reviewable |
| **Embedded DSL (Rust macro)** | Ties workflow definitions to the Rust compiler; external tools can't generate them |

## Trade-offs

- **YAML has footguns** — multi-line strings, type coercion, and alias merging cause subtle bugs. We mitigate this with strict schema validation and clear error messages.
- **ApprovalGate requires an external HTTP client.** There is no built-in CLI prompt mode. For local development, you can use `curl` to approve, but a TUI mode would be a useful addition.
- **ConfidenceRouter currently only supports hard threshold escalation.** A future version could support probabilistic routing based on historical success rates.

## For senior engineers / PEs

- **The executor is a state machine.** `WorkflowState` enum tracks: `Pending → Running → Paused (ApprovalGate) → Completed | Failed | Escalated`. Transitions are validated against the current state.
- **Retries use a dedicated `RetryState` struct** that tracks attempt count, next delay, and cumulative elapsed time. The `backoff_factor` is applied as: `delay = initial_delay_ms * backoff_factor^attempt`.
- **Snapshot records** include: execution ID, node ID, node kind, parent snapshot ID, timestamp, input/output payloads, and exit status. The chain is reconstructable from any snapshot by following parent links.
