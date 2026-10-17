#!/usr/bin/env bash
set -euo pipefail

echo "==> Running full agent-spine test suite..."
AUTONOMIC_BUDGET_GATE=0 cargo test -p agent-spine -- --nocapture

echo "==> Full agent-spine test suite passed."
