# agent-spine

`agent-spine` runs agent workflows as deterministic DAGs defined in YAML. Each run records immutable snapshots, so a failed execution can be replayed from any step.

Docs: [autonomic-ai-dev.vercel.app](https://autonomic-ai-dev.vercel.app)

## Quick start

```bash
brew tap autonomic-ai-dev/tap && brew install agent-spine
agent-spine init
agent-spine validate dev-pipeline.yaml
agent-spine run dev-pipeline.yaml
```

## Commands

| Command | What it does |
|---------|--------------|
| `agent-spine init` | Creates the config and an example workflow |
| `agent-spine doctor` | Checks prerequisites and configuration |
| `agent-spine validate <file>` | Checks a workflow against the schema, without running it |
| `agent-spine run <file>` | Runs a workflow |
| `agent-spine serve` | Starts the event bus and dashboard API on port 3100 |
| `agent-spine mcp-serve` | Starts the MCP stdio server only |

## HTTP routes

| Method | Path | Purpose |
| :--- | :--- | :--- |
| `GET` | `/api/v1/health` | Health check |
| `POST` | `/api/v1/workflows/run` | Run a workflow |
| `GET` | `/api/v1/executions/{id}` | Status of one execution |
| `POST` | `/api/v1/events` | Publish an event to the bus |
| `GET` | `/api/v1/events/subscribe` | Server-sent event stream. The optional `subject` query parameter filters it; the default `>` matches every subject |

## Develop

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Source builds need `protoc`. The dashboard needs `bun`, and the MCP bridge needs `agent-brain`. Both are optional.

Feature map and knowledge base: `.verify/features/` and `.verify/knowledge-base/`.
