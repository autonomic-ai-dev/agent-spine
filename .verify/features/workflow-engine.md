# workflow-engine

Summary: agent-spine loads a workflow YAML DAG, validates it against the schema, and executes its nodes in dependency order, including linear runs and fork-join branches.

## Sub-features

- `workflow-validate`: Checks a workflow YAML against the schema.
  - Intent: Reject malformed DAGs before anything runs.
  - Behaviour: `agent-spine validate <file>` parses the file and reports schema violations.
  - Outcome: Exit 0 for a valid file, non-zero with the violation for an invalid one.
- `workflow-run`: Executes a workflow with the built-in LocalAgent.
  - Intent: Run a pipeline from the command line.
  - Behaviour: `agent-spine run <file>` executes nodes in topological order.
  - Outcome: Each node runs once, in dependency order, and its result is recorded.
- `executor-linear`: Runs a chain of nodes one after another.
  - Intent: Guarantee a dependent node sees its predecessor's output.
  - Behaviour: Covered by `test_executor_linear_run` in `tests/engine_tests.rs`.
  - Outcome: The executed-node list equals the chain order.
- `executor-fork-join`: Runs parallel branches and waits for all of them.
  - Intent: Allow independent work to overlap while a join node waits for every branch.
  - Behaviour: Covered by `test_fork_join_barrier` and `test_fork_join_with_multi_level_branches`.
  - Outcome: Every branch executes before the join, and multi-level branches all execute.
- `dashboard-and-grpc`: Serves the gRPC API and dashboard. The gRPC default port is 3100 (`agent-spine/src/main.rs` line 174). The dashboard port defaults to port + 1.
  - Intent: Let the TUI and UI read workflow state.
  - Behaviour: `agent-spine serve` starts gRPC and the dashboard API.
  - Outcome: The server listens on the gRPC port, and the dashboard on the next port.
- `api-v1-http`: Runs workflows, reads executions, and publishes or subscribes to events over HTTP.
  - Intent: Give non-CLI callers a way to start and observe a workflow.
  - Behaviour: Routes in `agent-spine/src/autonomic_api.rs` and `src/event.rs`: `GET /api/v1/health`, `POST /api/v1/workflows/run`, `GET /api/v1/executions/{id}`, `POST /api/v1/events`, `GET /api/v1/events/subscribe`.
  - Outcome: Each route returns its documented response. Not verified by a test in this pass.

## How to get to it (user POV)

- CLI: `agent-spine validate <file>`, `agent-spine run <file>`, `agent-spine serve`
- MCP: `spine_submit_workflow`, `spine_check_status`, `spine_list_workflows` through the agent-spine MCP server
- Bridge: `agent-spine brain route <task>` and `agent-spine brain health`

## Verification Suites (Targeted Test Commands)

- **Unit Tests:** `cargo test` (whole crate), or the engine integration file `cargo test --test engine_tests`
- **Functional API Tests (Hurl):** not yet written for the gRPC or dashboard API.
- **Allowlisted Verification:** `.verify/run.sh` and the entry points in `.verify/verify.toml`
- **Preconditions:** Rust toolchain. `protoc` and `bun` are needed for the dashboard build, as `agent-spine doctor` reports.

### Verification Steps

1. **engine-integration** (sub-features: executor-linear, executor-fork-join)
   - Request: `cargo test --test engine_tests`
   - Why: Proves linear execution order and fork-join barriers.
   - Request (current): `sh .verify/checks/require_tests.sh --test engine_tests`. The guard fails on zero matched tests.
   - Expected: 9 passed, 0 failed. Verified 2026-10-24 by this command and by the 10-run check in Gotchas. The last guard output is in `.verify/artifacts/require-tests.log`.
   - Impact: A failure means node execution order is wrong. Fix the executor. Do not change the expected node lists.

## Gotchas

- Resolved flake (2026-10-24): the three baseline failures (`test_fork_join_barrier`, `test_fork_join_with_multi_level_branches`, `test_executor_linear_run`) were timing races, not executor logic. Each test slept a fixed 50 ms and then asserted the pending set. Under parallel load the executor had not dispatched the node yet. The fix replaces each sleep with `wait_for_pending`, which polls for up to 5 s and keeps the same expected values. Ten consecutive runs of `engine_tests` pass 9 of 9. The full spine suite passes 61 tests with 0 failed.
- Not verified: whether any test in the suite feeds `validate` an invalid file. Confirm before claiming invalid DAGs are rejected.
