#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/.." && pwd)"

echo "==> [thesmos] Starting verification suite..."
cd "$REPO_ROOT"

echo "==> Checking cargo clippy with -D warnings..."
cargo clippy -p thesmos -- -D warnings

echo "==> Checking cargo check..."
cargo check -p thesmos

echo "==> Running Playbook: Workflow Engine..."
"$DIR/playbooks/verify_workflow_engine.sh"

echo "==> Running Playbook: Full Suite..."
"$DIR/playbooks/verify_full_suite.sh"

echo "==> [thesmos] All verification playbooks passed cleanly (exit 0)."
