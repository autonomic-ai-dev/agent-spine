# 1. Problem and design goals

## What we built

agent-spine is a **deterministic workflow engine** for AI agent pipelines. It accepts a YAML-defined directed acyclic graph (DAG) of operations, executes them in order with parallel branching and human-in-the-loop approval gates, and records every state transition as an immutable snapshot.

## Why this way

Three observations drove the design:

1. **Shell scripts are not enough.** They have no DAG structure, no state machine, no parallelism, no approval gates, and no audit trail. Every team ends up building their own wrapper.
2. **CI/CD is not the answer.** GitHub Actions and similar tools are designed for push-triggered pipelines, not for agent-driven local workflows that need to pause for human input or dynamically select branches based on intermediate results.
3. **Determinism beats prompting for control flow.** Telling an LLM "run tests, then if they pass build, then deploy" is unreliable. The model might skip steps, reorder them, or hallucinate non-existent commands. A deterministic DAG guarantees execution order.

## Alternatives considered

| Option | Why we didn't choose it |
|--------|------------------------|
| **LangGraph / CrewAI** | Python runtime dependency, framework lock-in, not a local binary. agent-spine is a single Rust binary with no language runtime. |
| **Temporal.io / Cadence** | Production-grade but heavyweight — requires a server cluster. agent-spine is local-first with optional SQLite persistence. |
| **Makefiles / Justfiles** | Excellent task runners but no state machine, no approval gates, no snapshot replay, no parallel fan-out/fan-in. |
| **Custom script per project** | Every team reinvents the same patterns (retries, logging, state). agent-spine standardizes them once. |

## Trade-offs

- **YAML is verbose for simple pipelines.** A two-step "lint then build" pipeline requires more boilerplate than a shell script. We accept this because the structure pays off as pipelines grow.
- **LocalAgent resolves nodes in-process.** For multi-machine workflows, you need the gRPC serve mode and external agents. The in-process LocalAgent is a convenience for single-machine development.
- **Immutable snapshots mean storage grows with executions.** SQLite and JSONL stores are unbounded by default. Operators must configure retention policies for production.

## For senior engineers / PEs

- **Invariant:** Every execution produces a chain of immutable snapshots linked by parent references. No snapshot is ever modified after creation.
- **Failure mode:** If the executor process crashes mid-execution, in-progress snapshots may be lost (InMemory store) or partially written (JSONL/SQLite). Replay from the last complete snapshot is safe.
- **Scale question:** How does spine handle workflows with 10,000+ nodes or 1,000+ parallel branches? Current JoinSet-based fan-out works well for tens of branches; at hundreds, we would need a more sophisticated partition strategy.
- **Security boundary:** ApprovalGate nodes pause execution and wait for an external HTTP signal. The signal is authenticated but not encrypted by default — HTTPS should terminate at a reverse proxy in production deployments.
