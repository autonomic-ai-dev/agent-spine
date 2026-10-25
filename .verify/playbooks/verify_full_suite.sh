#!/usr/bin/env bash
set -euo pipefail

echo "==> Running full agent-spine test suite..."
cargo test -p agent-spine -- --nocapture

echo "==> Full agent-spine test suite passed."
