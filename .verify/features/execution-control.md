# execution-control

Summary: agent-spine controls how a workflow run stops, repeats, and recovers. It cancels on a signal, de-duplicates repeated requests with an idempotency store, tracks pending tasks in a supervisor that bridges state to NATS, and wakes agent-brain on demand when a call needs it.

## Sub-features

- `cancel-on-signal`: Lets a running workflow stop cleanly when the process receives a termination signal.
  - Intent: Avoid leaving nodes half-run after an operator interrupts the process.
  - Behaviour: `CancelToken` (`agent-spine/src/cancellation.rs` line 10) is shared with `setup_signal_handler` (line 80), which spawns a task that trips the token on the signal.
  - Outcome: The token reads as cancelled after the signal. How the executor reacts to a cancelled token is not verified in this pass.
- `idempotency-check`: Returns the stored result for a repeated request instead of running it again.
  - Intent: Make a retried node safe to run twice.
  - Behaviour: `check_idempotency` (`src/idempotency.rs` line 212) consults either `InMemoryIdempotencyStore` (line 45) or `SqliteIdempotencyStore` (line 92). Each stores an `IdempotencyRecord` (line 15). Failures surface as `IdempotencyError` (line 199).
  - Outcome: A repeated key returns the earlier record. A new key is recorded before the work runs. Not verified by a test in this pass.
- `supervisor-pending-tasks`: Tracks every pending task and emits workflow events.
  - Intent: Let operators and the TUI see what is running, and let a restarted process know what was pending.
  - Behaviour: `Supervisor` (`src/supervisor.rs` line 70) holds `PendingTaskMeta` (line 50) and `PendingTaskInfo` (line 60), and emits `WorkflowEvent` (line 10). Errors are `SupervisorError` (line 270).
  - Outcome: Pending tasks appear in the supervisor's state. Not verified by a test in this pass.
- `state-bridge-to-nats`: Publishes supervisor state to NATS.
  - Behaviour: `spawn_state_bridge` (`src/jetstream_bridge.rs` line 95) takes the supervisor and the NATS URL. `jetstream_bridge.rs` has 2 inline tests.
  - Outcome: Events reach the bus that agent-tui and agent-ui read. Not verified against a live broker in this pass.
- `wake-brain-on-call`: Starts agent-brain when a call needs it and it is not running.
  - Behaviour: `try_spawn_brain` (`src/wake_on_call.rs` line 48) starts the process. `wait_for_brain` (line 65) polls up to `max_attempts`.
  - Outcome: `true` when brain answers, `false` after the attempts run out. No test exists for this module.

## How to get to it (user POV)

- Signal: Ctrl-C or SIGTERM sent to a running `agent-spine run` or `agent-spine serve` process.
- NATS: the supervisor state bridge publishes on the event bus when `serve` is running.
- Automatic: idempotency and brain wake-up run inside `agent-spine run` and `serve`. They have no separate command.

## Verification Suites (Targeted Test Commands)

- **Unit Tests:** `sh .verify/checks/require_tests.sh --test execution_control_tests` (9 tests: idempotency with in-memory and SQLite stores, cancellation, supervisor delegate and resume). `wake_on_call.rs` has no test.
- **Functional API Tests (Hurl):** not written.
- **Allowlisted Verification:** entries in `.verify/verify.toml`
- **Preconditions:** Rust toolchain. Whether the `jetstream_bridge.rs` tests need a live NATS broker was not checked in this pass.

### Verification Steps

1. **idempotency-unit-tests** (sub-feature: idempotency-check)
   - Request: `sh .verify/checks/require_tests.sh --test execution_control_tests`
   - Why: Proves a repeated key returns the stored record, that SQLite keys survive a reopen, that cancellation latches and notifies, and that a duplicate delegate is rejected.
   - Expected: 9 passed, 0 failed. The guard fails if a filter matches zero tests.
   - Impact: A failure means a retried node could run twice. Add the test before relying on retries.

## Gotchas

- Resolved: idempotency, cancellation, and supervisor now have tests in `tests/execution_control_tests.rs`. The idempotency error path (`IdempotencyError`) is not exercised yet.
- Gap: `wake_on_call.rs` has no test. `wait_for_brain` depends on whether a brain answers on this machine, so a test needs a fixed port and a stub server.
- Not verified: whether the executor checks `CancelToken` between nodes, or only at process exit.
- `wait_for_brain` returns `false` after its attempts run out. Callers must handle that result, or a missing brain looks like a successful wake-up.
