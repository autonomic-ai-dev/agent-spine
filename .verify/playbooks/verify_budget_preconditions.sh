#!/usr/bin/env bash
set -euo pipefail

echo "==> Running budget precondition and workflow validation tests..."
cargo test -p agent-spine --lib workflow::tests -- --nocapture
cargo test -p agent-spine --lib budget_gate::tests -- --nocapture

echo "==> Budget precondition verification passed."
