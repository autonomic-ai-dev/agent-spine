# Workflow engine

## What it does

agent-spine loads a workflow YAML file, a directed acyclic graph of nodes, validates it against the schema, and runs its nodes in dependency order. It supports linear chains and fork-join branches.

## How it works

1. **Validate.** `agent-spine validate <file>` parses the YAML and reports schema violations. A valid file exits 0. An invalid one exits non-zero with the violation.
2. **Run.** `agent-spine run <file>` executes nodes in topological order. Each node runs once, and its result is recorded.
3. **Linear chains.** A node waits for its predecessor. `test_executor_linear_run` checks that the executed-node order matches the chain.
4. **Fork-join.** Parallel branches run together, and a join node waits for every branch. Multi-level branches also finish before the join.
5. **Serve.** `agent-spine serve` starts gRPC on port 3100 and the dashboard on the next port.
6. **HTTP API.** Routes in `autonomic_api.rs` and `event.rs`:
   - `GET /api/v1/health`
   - `POST /api/v1/workflows/run`
   - `GET /api/v1/executions/{id}`
   - `POST /api/v1/events`
   - `GET /api/v1/events/subscribe`, a server-sent event stream. The `subject` query parameter filters it, and the default `>` matches every subject.
7. **MCP.** `spine_submit_workflow`, `spine_check_status`, and `spine_list_workflows`, served by agent-spine.

## Where the code is

- Executor and engine: `agent-spine/src/` (`workflow.rs`, `executor/`)
- HTTP routes: `src/autonomic_api.rs` and `src/event.rs`
- Tests: `tests/engine_tests.rs`

## How to use it

- `agent-spine validate workflow.yaml`, then `agent-spine run workflow.yaml`
- `agent-spine serve` for the API and dashboard
- `curl -X POST .../api/v1/workflows/run` for a run over HTTP

## How it is verified

- `sh .verify/checks/require_tests.sh --test engine_tests`: 9 tests, all passing. The timing races were fixed by replacing fixed sleeps with a bounded `wait_for_pending` poll.
- `bash .verify/checks/route_drift.sh`: the README's HTTP table matches the `.route()` calls in source

## Known limits

- The HTTP response bodies have no test. Routes are checked for existence only.
- Whether `validate` rejects an invalid file is not confirmed by any test.

## Related

- Execution control: [execution-control](execution-control.md)