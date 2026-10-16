#!/usr/bin/env bash
# Build agent-spine release and Developer ID–sign on macOS (Apple Silicon).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release -p agent-spine
"$ROOT/scripts/sign-macos.sh"
