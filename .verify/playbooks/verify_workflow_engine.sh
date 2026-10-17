#!/usr/bin/env bash
set -euo pipefail

echo "==> Running DAG workflow engine and state transitions tests..."
AUTONOMIC_BUDGET_GATE=0 cargo test -p agent-spine --test engine_tests -- --nocapture
AUTONOMIC_BUDGET_GATE=0 cargo test -p agent-spine --test workflow_definition -- --nocapture
AUTONOMIC_BUDGET_GATE=0 cargo test -p agent-spine --test workflow_state -- --nocapture

echo "==> Workflow engine verification passed."
