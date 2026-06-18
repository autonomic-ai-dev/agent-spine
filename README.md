# Agent Spine

Agent Spine is a local-first, stateful workflow engine for supervising AI coding agents. It provides deterministic graph execution, immutable state history, and native IDE hooks while [`agent-brain`](https://github.com/aeswibon/agent-brain) is intended to supply token-efficient context for each node.

The project is **not** another terminal-bound agent crew. It targets a lightweight Rust supervisor that integrates with tools like Claude Code and Cursor, runs declarative YAML workflows, and records every state transition for inspection, replay, and intervention.

> **Status:** Phase 1–2 in progress. The execution engine, state stores, gRPC supervisor, confidence router, parallel fan-out/fan-in, and a Svelte dashboard are implemented. CLI `run`/`replay`/`inspect`, agent-brain routing, and production persistence adapters are not yet wired end-to-end.

## Why Agent Spine?

Existing orchestration frameworks are capable, but local production workflows often hit the same operational costs:

- graph definitions accumulate infrastructure boilerplate;
- cyclic failures are difficult to diagnose because state changes are opaque;
- execution is separated from the IDE agent interface where developers already work;
- retries and multi-agent reasoning can become unbounded or impossible to audit.

Agent Spine treats these as execution-engine problems. Workflows should be declarative, transitions immutable, verification mechanical, and escalation explicit.

## Architecture

Agent Spine is organized around four layers:

1. **Information** — `agent-brain` selects scoped context for each node *(planned integration)*.
2. **Structure** — Agent Spine decomposes work into typed, bounded transitions.
3. **Reasoning depth** — optional candidate generation, debate, voting, and tree search trade compute for confidence *(planned)*.
4. **Mechanical verification** — compilers, tests, linters, schemas, and reward models gate state transitions *(partial)*.

```text
┌─────────────────────────────────────────────────────────────┐
│  Dashboard (Svelte + Connect/gRPC-Web)                      │
│  list executions · view history · resume pending tasks      │
└───────────────────────────┬─────────────────────────────────┘
                            │ gRPC / gRPC-Web
┌───────────────────────────▼─────────────────────────────────┐
│  Supervisor API          │  Dashboard API                     │
│  resume · pending tasks  │  list · history                    │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  Executor                                                   │
│  state machine · parallel fan-out/fan-in · retries/backoff  │
│  ApprovalGate · ConfidenceRouter escalation                 │
└───────┬───────────────────────────────┬─────────────────────┘
        │                               │
┌───────▼────────┐              ┌───────▼────────┐
│  Supervisor  │              │  WorkflowState │
│  IDE hooks   │              │  InMemory      │
│  pause/resume│              │  File (JSONL)  │
└──────────────┘              │  SQLite        │
                              └────────────────┘
```

### Core modules

| Module | Role |
|--------|------|
| `workflow` | YAML workflow definitions, validation, `NodeKind` (`Agent`, `Checkpoint`, `Verify`, `ApprovalGate`) |
| `executor` | Async state-machine traversal with parallel branches, fan-in merge, retries, and routing |
| `supervisor` | Pauses agent nodes and waits for IDE `resume()` via gRPC |
| `router` | `ConfidenceRouter` — tracks verification failures and sets `escalation_required` |
| `state` | Append-only `WorkflowState` trait with in-memory, JSONL file, and SQLite adapters |
| `api` | gRPC services generated from `proto/supervisor.proto` |
| `dashboard/` | Svelte frontend for execution monitoring and HITL resume |

Rust is the primary implementation language for predictable resource use, safe concurrency, durable local execution, and a single distributable binary. Model providers, IDE integrations, and persistence backends remain behind adapter traits.

## Current capabilities

### Implemented

- **Declarative YAML workflows** with versioned schemas and validation
- **Immutable append-only snapshots** with parent linkage and monotonic sequence numbers
- **Parallel fan-out / fan-in** via multiple outgoing edges and `tokio::task::JoinSet`
- **Human-in-the-loop gates** via `ApprovalGate` nodes and supervisor resume
- **Built-in retries** with exponential backoff before supervisor failure (hardcoded policy today)
- **Confidence routing** after repeated verification failures
- **State stores**: `InMemoryStateStore`, `FileStateStore` (JSONL), `SqliteStateStore`
- **gRPC supervisor + dashboard API** with gRPC-Web and CORS for browser clients
- **Live dashboard** (Svelte) — execution list, history viewer, pending-task resume
- **CI/CD** via `pipeline-compose` — Rust check, multi-platform release builds, tag publish

### Not yet implemented

- CLI `run`, `inspect`, and `replay` commands
- Wiring the executor into `agent-spine serve` (server exposes APIs but does not execute workflows today)
- `agent-brain` context routing per node
- Postgres / Redis state adapters
- OpenTelemetry export (`tracing` spans exist; no OTel pipeline)
- Configurable `RetryPolicy` in workflow YAML
- WebSocket streaming for live DAG updates (dashboard polls today)
- Native Claude Code / Cursor IDE adapters
- Example workflow YAML files in-repo

## Repository audit

Audit date: 2026-06-18. Compared against the Superpower Orchestrator implementation plan and the next-phase roadmap.

### Phase progress

| Phase | Goal | Status | Notes |
|-------|------|--------|-------|
| **1 — Executable Core** | YAML, DAG execution, snapshots, CLI | **~70%** | Engine + stores + tests exist; CLI lacks `run`/`replay`/`inspect` |
| **2 — Native IDE Supervision** | Supervisor hook protocol, pause/resume | **~60%** | gRPC supervisor works; no JSONL hook protocol or IDE-specific adapters |
| **3 — Agent-Brain Integration** | Route nodes through agent-brain | **0%** | Not started |
| **4 — Inference Scaling** | Debate, voting, verification loops | **~15%** | `ConfidenceRouter` + `Verify` node kind only |
| **5 — Production Hardening** | Migrations, OTel, leases, redaction | **~25%** | SQLite store + tracing; no OTel, cancellation, or idempotency keys |

### Roadmap feature assessment

| Feature | Current state | Gap |
|---------|---------------|-----|
| **Parallel fan-out / fan-in** | Working in `executor.rs` via edge topology | No explicit fork/join node kinds; payload merge is shallow JSON merge |
| **Production state stores** | SQLite + JSONL file store | Postgres (`sqlx` JSONB) and Redis adapters missing |
| **Human-in-the-loop wait states** | `ApprovalGate` + supervisor resume | 30s supervisor timeout; no webhook/event on suspend; not indefinite wait |
| **Observability / tracing** | `tracing` instrumentation on key paths | No `tracing-opentelemetry`; no Jaeger/Datadog export |
| **Retries with backoff** | Hardcoded 3 retries, exponential backoff in executor | Not declarative in workflow YAML; failures still recorded per retry attempt in supervisor path |
| **Live dashboard** | Svelte + gRPC-Web dashboard exists | Polling-based; no WebSocket stream; executor not connected to server |

### Strengths

- Clean trait boundaries (`WorkflowState`, supervisor delegation)
- Immutable snapshot model with sequence enforcement across all stores
- Parallel execution tests cover linear, fan-out/fan-in, approval gates, and router escalation
- gRPC API surface is stable and dashboard-ready
- CI matches the `agent-brain` repo pattern (`pipeline-compose`, stage builds, cross-platform matrix)

### Risks and gaps

1. **`serve` does not run workflows** — the dashboard can list history and resume tasks, but nothing starts an `Executor` from the server process today.
2. **Duplicate gRPC layer** — `server.rs` duplicates supervisor handlers already in `api.rs` and is unused.
3. **No sample workflows** — no checked-in YAML under `examples/` for onboarding.
4. **Supervisor timeout** — 30-second default conflicts with true HITL “wait indefinitely” semantics.
5. **Escalation is advisory** — router sets `escalation_required` in payload but does not invoke a frontier provider.
6. **README was stale** — previously described an early skeleton; implementation has moved significantly.

### Recommended next steps

Priority order for the highest capability boost:

1. **Wire executor into `serve`** — load YAML, start execution, persist to SQLite, expose live state to dashboard.
2. **CLI `run` / `inspect` / `replay`** — complete Phase 1 exit criteria.
3. **Formalize HITL** — indefinite wait on `ApprovalGate`, webhook/gRPC event on suspend, configurable timeout elsewhere.
4. **Declarative `RetryPolicy`** on `WorkflowNode` in YAML.
5. **OpenTelemetry** — export spans for state transitions and node execution.
6. **Postgres adapter** — when concurrent workflow volume exceeds SQLite comfort zone.

## Project layout

```text
.
├── agent-spine/              # Rust workspace member (engine + CLI)
│   ├── proto/                # gRPC service definitions
│   ├── src/
│   │   ├── workflow.rs       # YAML schema + validation
│   │   ├── executor.rs       # State machine + parallel execution
│   │   ├── supervisor.rs     # IDE pause/resume
│   │   ├── router.rs         # Confidence routing
│   │   ├── state.rs          # InMemory, File, SQLite stores
│   │   ├── api.rs            # gRPC service implementations
│   │   └── main.rs           # CLI: status, validate, serve
│   └── tests/                # Engine, workflow, state tests
├── dashboard/                # Svelte + Connect/gRPC-Web UI
├── .github/
│   ├── pipelines/            # pipeline-compose stage definitions
│   └── workflows/            # CI, release, multi-platform build
├── Cargo.toml                # Workspace root
└── README.md
```

## Getting started

### Prerequisites

- Rust stable with Cargo
- [protobuf compiler](https://grpc.io/docs/protoc-installation/) (for gRPC code generation)
- [Bun](https://bun.sh/) (for dashboard development)

### Build and test

```bash
git clone https://github.com/aeswibon/agent-spine.git
cd agent-spine
cargo test --workspace --all-features
cargo run -p agent-spine -- validate path/to/workflow.yaml
```

### Run the dashboard server

```bash
cargo run -p agent-spine -- serve --db state.db --port 3000
```

### Dashboard development

```bash
cd dashboard
bun install
bun run dev      # dev server (expects gRPC backend on :3000)
bun run check    # type check
bun run build    # production build
```

### Development checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Delivery phases

### Phase 1 — Executable Core

- Define versioned workflow schemas
- Parse and validate YAML workflow definitions
- Implement deterministic DAG execution
- Persist immutable snapshots and edge transitions
- Add `run`, `validate`, `inspect`, and `replay` CLI commands

**Exit criteria:** a sample workflow executes locally and can be replayed to any transition.

### Phase 2 — Native IDE Supervision

- Define a stable supervisor hook protocol over JSON Lines
- Add Claude Code and Cursor adapters
- Stream node status and structured intervention requests
- Support pause, approve, reject, retry, and resume operations

**Exit criteria:** a developer can observe and intervene without leaving the IDE agent flow.

### Phase 3 — Agent-Brain Integration

- Route each node through `agent-brain` before execution
- Record context IDs and token budgets in snapshot metadata
- Feed outcome usefulness back to the context router
- Prevent secrets and oversized payloads from entering persisted state

### Phase 4 — Inference Scaling and Verification

- Configurable candidate generation and self-consistency voting
- Implementer/reviewer debate policies
- Compiler, test, lint, and schema verification loops
- Confidence routing after bounded repeated failures

### Phase 5 — Production Hardening

- SQLite migrations, crash recovery, Postgres adapter
- Cancellation, leases, idempotency, and concurrency limits
- OpenTelemetry-compatible traces and local visualization
- Redaction policies, signed audit records, and plugin capability controls

## Key invariants

1. State snapshots are immutable and append-only.
2. Every transition references its parent snapshot.
3. Workflow and state schemas are explicitly versioned.
4. Retries, debates, and searches have hard execution and token limits.
5. External effects use idempotency keys and are recorded before acknowledgement.
6. Human approval is required for configured high-impact transitions.
7. Provider-specific behavior remains behind adapter traits.
8. Replay creates a new execution branch; it does not rewrite history.

## First public milestone

The first milestone proves one narrow path:

1. Load a YAML workflow.
2. Validate nodes, edges, and typed input/output schemas.
3. Execute mock agent nodes supervised via gRPC.
4. Persist each state transition to SQLite.
5. Inspect execution history from the CLI and dashboard.
6. Replay from a selected snapshot into a new execution branch.

This deliberately excludes MCTS, frontier escalation, and full IDE integrations until the deterministic state model is stable.

## Contributing

Issues that sharpen the workflow schema, snapshot model, IDE hook protocol, or verification boundaries are especially welcome.

Before submitting a change:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
