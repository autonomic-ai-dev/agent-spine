# Execution control

## What it does

agent-spine controls how a workflow run stops, repeats, and recovers. It cancels on a termination signal, de-duplicates repeated requests with an idempotency store, tracks pending tasks in a supervisor, bridges that state to NATS, and starts agent-brain when a call needs it and it is not running.

## How it works

1. **Cancel.** A `CancelToken` (`src/cancellation.rs`) is shared with a signal handler (`setup_signal_handler`). Ctrl-C or SIGTERM trips the token. Tests cover that the token latches and notifies.
2. **Idempotency.** `check_idempotency` (`src/idempotency.rs`) looks up a key in either an in-memory store or a SQLite store. A repeated key returns the stored record. A new key is recorded before the work runs, so a retry is safe.
3. **Supervisor.** `Supervisor` (`src/supervisor.rs`) holds pending-task metadata and emits `WorkflowEvent` values. Duplicate delegation is rejected, and a delegated task can be resumed.
4. **State bridge.** `spawn_state_bridge` (`src/jetstream_bridge.rs`) publishes supervisor state on NATS, where agent-tui and agent-ui read it.
5. **Wake brain.** `try_spawn_brain` (`src/wake_on_call.rs`) starts agent-brain. `wait_for_brain` polls up to `max_attempts` and returns `false` when they run out.

## Where the code is

- `src/cancellation.rs`, `src/idempotency.rs`, `src/supervisor.rs`, `src/jetstream_bridge.rs`, `src/wake_on_call.rs`
- Tests: `tests/execution_control_tests.rs` (9 tests), `tests/wake_on_call_tests.rs` (1 test)

## How to use it

- Interrupt `agent-spine run` or `agent-spine serve` with Ctrl-C. Nothing else is needed.
- Idempotency and brain wake-up run inside `run` and `serve`. They have no command of their own.

## How it is verified

- `sh .verify/checks/require_tests.sh --test execution_control_tests`: 9 passed
- `sh .verify/checks/require_tests.sh --test wake_on_call_tests`: 1 passed. This covers only the zero-attempt path.

## Known limits

- The `IdempotencyError` path is not exercised.
- `try_spawn_brain` with the `BRAIN_PATH` override is untested. Setting that variable needs `unsafe` environment mutation, which this crate forbids. A test needs the override passed as an argument.
- `wait_for_brain` with attempts above zero depends on whether a brain runs on the machine, so it is untested.
- Whether the executor checks the cancel token between nodes, or only at exit, is not verified.
- `wait_for_brain` returns `false` on failure. Callers must treat that as a failure, or a missing brain looks like a successful wake-up.
- The state bridge has no test against a live broker.

## Related

- Workflow engine: [workflow-engine](workflow-engine.md)
