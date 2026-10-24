# Workflow engine

## What it does

agent-spine loads a workflow YAML file, a directed acyclic graph of nodes, validates it against the schema, and runs its nodes in dependency order. It supports linear chains and fork-join branches. Before an LLM-heavy node runs, it asks agent-heart for budget approval.

## How it works

1. **Validate.** `agent-spine validate <file>` parses the YAML and reports schema violations. A valid file exits 0. An invalid one exits non-zero with the violation.
2. **Run.** `agent-spine run <file>` executes nodes in topological order. Each node runs once, and its result is recorded.
3. **Linear chains.** A node waits for its predecessor. `test_executor_linear_run` checks that the executed-node order matches the chain.
4. **Fork-join.** Parallel branches run together, and a join node waits for every branch. Multi-level branches also finish before the join.
5. **Budget gate.** Before an LLM node, spine calls `POST /budget/check` on agent-heart. An allowed node runs. A denied node halts the workflow.
6. **Serve.** `agent-spine serve` starts gRPC on port 3100 and the dashboard on the next port.
7. **HTTP API.** Routes in `autonomic_api.rs` and `event.rs`:
   - `GET /api/v1/health`
   - `POST /api/v1/workflows/run`
   - `GET /api/v1/executions/{id}`
   - `POST /api/v1/events`
   - `GET /api/v1/events/subscribe`, a server-sent event stream. The `subject` query parameter filters it, and the default `>` matches every subject.
8. **MCP.** `spine_submit_workflow`, `spine_check_status`, and `spine_list_workflows`, through agent-body.

## Where the code is

- Executor and engine: `agent-spine/src/` (`workflow.rs`, `executor/`)
- Budget check: `agent-spine/src/budget_gate.rs` (`check_node`), with the runner's halt in `agent-spine/src/executor/runner.rs`
- HTTP routes: `src/autonomic_api.rs` and `src/event.rs`
- Tests: `tests/engine_tests.rs`

## How to use it

- `agent-spine validate workflow.yaml`, then `agent-spine run workflow.yaml`
- `agent-spine serve` for the API and dashboard
- `curl -X POST .../api/v1/workflows/run` for a run over HTTP

## How it is verified

- `sh .verify/checks/require_tests.sh --test engine_tests`: 9 tests, all passing. The timing races were fixed by replacing fixed sleeps with a bounded `wait_for_pending` poll.
- `bash .verify/checks/route_drift.sh`: the README's HTTP table matches the `.route()` calls in source
- `sh .verify/checks/require_tests.sh budget_gate`: 17 tests, including the fail-closed cases for heart

## Known limits

- The per-node budget check fails closed (plan item 3.3). If heart is unreachable, returns an HTTP error, an unparseable body, an error response, or no decision, the node is denied and the workflow halts. Only an approved decision lets it run. The precondition checks (headroom and quota) also fail closed.
- The halt is covered at workflow level. `tests/budget_halt_tests.rs` runs a one-agent workflow against a fake heart. A 500 answer returns `ExecutorError::BudgetUnavailable` and the node is never dispatched. An approval dispatches it.
- The full-suite run sets `AUTONOMIC_BUDGET_GATE=0`, so it never exercises the per-node gate. The `budget_gate` guard runs those tests.
- The HTTP response bodies have no test. Routes are checked for existence only.
- Whether `validate` rejects an invalid file is not confirmed by any test.

## Related

- Execution control: [execution-control](execution-control.md)
- Budget gate on the heart side: [budget-gate](../../../agent-heart/.verify/knowledge-base/budget-gate.md)
