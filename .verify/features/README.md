# agent-spine features

## Baseline preconditions

- Rust toolchain matching `Cargo.lock`. `protoc` and `bun` are needed for the dashboard, as `agent-spine doctor` reports.
- The budget-gate check needs agent-heart on port 3101 when an LLM node runs.
- gRPC and dashboard checks need `agent-spine serve` on port 3100.

## Driving conventions

- Unit and integration tests run through `cargo test`.
- Logs go to `.verify/artifacts/<RUN_ID>/`. Baseline logs are in `.verify/artifacts/feature-map-baseline/`.

## Proof standards

- A workflow feature is proven when its engine test passes and its validate or run command exits with the expected status on a fixture workflow.

## Feature index

| Feature file | Sub-features | Primary surface |
|---|---|---|
| `workflow-engine.md` | 7 | `validate`, `run`, `serve` (gRPC on 3100), `/api/v1/*` HTTP, MCP tools |
| `execution-control.md` | 5 | Ctrl-C cancel, idempotency store, supervisor state bridge, brain wake-up. Idempotency has no test. |
