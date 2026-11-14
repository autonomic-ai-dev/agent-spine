#!/usr/bin/env bash
set -euo pipefail

echo "==> Running full thesmos test suite..."
cargo test -p thesmos -- --nocapture

echo "==> Full thesmos test suite passed."
