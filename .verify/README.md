# agent-spine Verification Suite

Verification playbooks and automated test suites for `agent-spine` (the Autonomic AI stateful workflow DAG and supervision engine).

## Structure
- `run.sh`: Master entrypoint executing clippy, check, and all modular playbooks.
- `verify.toml`: Machine-readable descriptor of test commands.
- `playbooks/`:
  - `verify_workflow_engine.sh`: Tests DAG execution, fork/join barriers, and state transitions.
  - `verify_full_suite.sh`: Comprehensive test suite across unit tests and integration tests.
