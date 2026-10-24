# workflow-engine

Summary: agent-spine loads a workflow YAML DAG, validates it against the schema, and executes its nodes in dependency order, including linear runs and fork-join branches, with an LLM budget check before LLM-heavy nodes.

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
- `budget-gate-node`: Asks agent-heart whether an LLM node may run.
  - Intent: Stop an LLM-heavy node before it spends tokens over the ceiling.
  - Behaviour: Before an LLM node, spine calls `POST /budget/check` on agent-heart.
  - Outcome: Allowed nodes run. Denied nodes halt the workflow.
- `dashboard-and-grpc`: Serves the gRPC API and dashboard. The gRPC default port is 3100 (`agent-spine/src/main.rs` line 174). The dashboard port defaults to port + 1.
  - Intent: Let the TUI and UI read workflow state.
  - Behaviour: `agent-spine serve` starts gRPC and the dashboard API.
  - Outcome: The server listens on the gRPC port, and the dashboard on the next port.
- `api-v1-http`: Runs workflows, reads executions, and publishes or subscribes to events over HTTP.
  - Intent: Give non-CLI callers a way to start and observe a workflow.
  - Behaviour: Routes in `agent-spine/src/autonomic_api.rs` and `src/event.rs`: `GET /api/v1/health`, `POST /api/v1/workflows/run`, `GET /api/v1/executions/{id}`, `POST /api/v1/events`, `GET /api/v1/events/subscribe`.
  - Outcome: The routes are listed in the README's HTTP routes table, and `.verify/checks/route_drift.sh` checks that each one resolves to a `.route()` call in source. The response bodies are not verified by a test in this pass.

## Code

- `agent-spine/src/agent.rs`
- `agent-spine/src/api.rs`
- `agent-spine/src/async_sandbox.rs`
- `agent-spine/src/autonomic_api.rs`
- `agent-spine/src/brain_router.rs`
- `agent-spine/src/budget_gate.rs`
- `agent-spine/src/condition.rs`
- `agent-spine/src/event.rs`
- `agent-spine/src/execution_trace.rs`
- `agent-spine/src/execution.rs`
- `agent-spine/src/executor`
- `agent-spine/src/global_workspace.rs`
- `agent-spine/src/jetstream.rs`
- `agent-spine/src/lib.rs`
- `agent-spine/src/log.rs`
- `agent-spine/src/main.rs`
- `agent-spine/src/mcp_bridge.rs`
- `agent-spine/src/mcp_server.rs`
- `agent-spine/src/meta_router.rs`
- `agent-spine/src/muscle_exec.rs`
- `agent-spine/src/registry_workflow.rs`
- `agent-spine/src/router.rs`
- `agent-spine/src/sandbox.rs`
- `agent-spine/src/setup.rs`
- `agent-spine/src/snapshot.rs`
- `agent-spine/src/speculative_hydrate.rs`
- `agent-spine/src/state.rs`
- `agent-spine/src/transition.rs`
- `agent-spine/src/update.rs`
- `agent-spine/src/workflow.rs`
- `agent-spine/src/workflow_manager.rs`
- `agent-spine/src/workflows.rs`
- `agent-spine/Cargo.toml`

## How to get to it (user POV)

- CLI: `agent-spine validate <file>`, `agent-spine run <file>`, `agent-spine serve`
- MCP: `spine_submit_workflow`, `spine_check_status`, `spine_list_workflows` through agent-body
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
2. **route-drift** (sub-features: api-v1-http)
   - Request: `bash .verify/checks/route_drift.sh`
   - Why: Proves each README route row matches a `.route()` in source, and each source route has a README row.
   - Expected: `route drift: README routes and src routes agree`.
   - Impact: A failure means a route was added, renamed, or removed without its README row. Fix whichever side is wrong.
3. **budget-gate-fail-closed** (sub-feature: budget-gate-node)
   - Request: `sh .verify/checks/require_tests.sh budget_gate`
   - Why: Proves each unavailable heart case (unreachable, HTTP error, unparseable body, error response, missing decision) is denied, and the approved case passes.
   - Expected: 17 passed, 0 failed.
   - Impact: A failure means a node could run while heart is unavailable. Fix the gate. Do not loosen the test.

## Gotchas

- Resolved flake (2026-10-24): the three baseline failures (`test_fork_join_barrier`, `test_fork_join_with_multi_level_branches`, `test_executor_linear_run`) were timing races, not executor logic. Each test slept a fixed 50 ms and then asserted the pending set. Under parallel load the executor had not dispatched the node yet. The fix replaces each sleep with `wait_for_pending`, which polls for up to 5 s and keeps the same expected values. Ten consecutive runs of `engine_tests` pass 9 of 9. The full spine suite passes 61 tests with 0 failed.
- The per-node budget check `check_node` (`agent-spine/src/budget_gate.rs`) fails closed (plan item 3.3). An unreachable heart, an HTTP error, an unparseable body, an error response, or a missing decision returns `BudgetGateError::Unavailable`, and the runner halts the workflow with `ExecutorError::BudgetUnavailable`. A frozen decision keeps its first-offense warning, then blocks. Six tests cover the unavailable cases and the approved case (`node_is_denied_when_heart_is_unreachable` and the others in the `budget_gate` tests). The halt itself is the runner's `?` propagation, with no workflow-level test yet.
- The full-suite entry (`AUTONOMIC_BUDGET_GATE=0 cargo test -p agent-spine`) turns the per-node gate off, so the suite never exercises it. The `budget_gate` guard runs the gate tests separately, through `require_tests.sh`.
- The budget gate needs agent-heart on port 3101. `agent-spine/src/budget_gate.rs` defines the error "budget unavailable for node … heart did not report headroom", so an unreachable or silent heart is treated as a denial, not an allow. This comes from the error type. A test that proves the halt behaviour has not been run.
- Not verified: whether any test in the suite feeds `validate` an invalid file. Confirm before claiming invalid DAGs are rejected.
