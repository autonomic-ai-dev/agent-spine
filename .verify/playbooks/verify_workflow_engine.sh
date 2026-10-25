#!/usr/bin/env bash
set -euo pipefail

echo "==> Running DAG workflow engine and state transitions tests..."
cargo test -p agent-spine --test engine_tests -- --nocapture
cargo test -p agent-spine --test workflow_definition -- --nocapture
cargo test -p agent-spine --test workflow_state -- --nocapture

echo "==> Workflow engine verification passed."
