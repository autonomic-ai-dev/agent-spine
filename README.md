# Agent Spine

Agent Spine is a local-first, stateful workflow engine for supervising AI coding agents. It provides the execution structure around native IDE agents while [`agent-brain`](https://github.com/aeswibon/agent-brain) supplies token-efficient context for each step.

The project is intentionally not another terminal-bound “agent crew.” Its target is a lightweight supervisor that integrates with tools such as Claude Code and Cursor, runs declarative workflows, and records every state transition for inspection, replay, and intervention.

> Status: early Rust skeleton. The state contracts and CLI shell exist; workflow parsing, durable persistence, and IDE adapters are not implemented yet.

## Why Agent Spine?

Existing orchestration frameworks are capable, but local production workflows often encounter the same operational costs:

- graph definitions accumulate infrastructure boilerplate;
- cyclic failures are difficult to diagnose because state changes are opaque;
- execution is separated from the IDE agent interface where developers already work;
- retries and multi-agent reasoning can become unbounded or impossible to audit.

Agent Spine treats these as execution-engine problems. Workflows should be declarative, transitions immutable, verification mechanical, and escalation explicit.

## Architectural Direction

Agent Spine is designed around four layers:

1. **Information** — `agent-brain` selects exact context for each node.
2. **Structure** — Agent Spine decomposes work into typed, bounded transitions.
3. **Reasoning depth** — optional candidate generation, debate, voting, and tree search trade compute for confidence.
4. **Mechanical verification** — compilers, tests, linters, schemas, and reward models gate state transitions.

Rust is the primary implementation language because the engine needs predictable resource use, safe concurrency, durable local execution, and a single distributable binary. Model providers, IDE integrations, and persistence backends will remain behind narrow adapter traits.

## Planned Capabilities

- Declarative YAML workflows with versioned schemas
- Immutable, append-only execution snapshots
- State inspection and time-travel replay
- Native IDE supervisor hooks
- Human-in-the-loop checkpoints
- Bounded retry and verification policies
- Multi-agent implementer/reviewer debate
- Confidence-based escalation for isolated failed tasks
- Tight `agent-brain` context routing
- Local-first operation with optional frontier-provider adapters

## Current Skeleton

```text
.
├── src/
│   ├── execution.rs   # execution identity
│   ├── snapshot.rs    # immutable state snapshots
│   ├── state.rs       # append-only state-store contract
│   ├── transition.rs  # graph-edge representation
│   ├── lib.rs         # public engine contracts
│   └── main.rs        # CLI entry point
├── tests/
│   └── workflow_state.rs
├── Cargo.toml
└── README.md
```

Local implementation plans are kept under `docs/superpowers/` and intentionally excluded from Git so that the public repository contains stable product documentation rather than task-local planning artifacts.

## Getting Started

Prerequisites:

- Rust stable with Cargo

```bash
git clone https://github.com/aeswibon/agent-spine.git
cd agent-spine
cargo test
cargo run -- status
```

Recommended development checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Initial Milestone

The first executable milestone will:

1. load and validate a YAML workflow;
2. execute deterministic mock-agent nodes;
3. persist transitions to SQLite;
4. inspect execution history through the CLI;
5. branch a new execution from any prior snapshot.

Advanced inference scaling and production IDE integrations will follow after the state and replay model is stable.

## Design Constraints

- State is immutable and append-only.
- Retries and inference scaling always have hard limits.
- External effects require idempotency keys.
- Provider-specific code stays outside the core engine.
- Human approval gates are first-class workflow nodes.
- Secrets must never be persisted in workflow payloads.
- Replay creates a new execution branch; it does not rewrite history.

## Contributing

The project is at the contract-design stage. Issues that sharpen the workflow schema, snapshot model, IDE hook protocol, or verification boundaries are especially useful.

Before submitting a change:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).

