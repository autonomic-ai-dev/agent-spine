#!/bin/sh
set -eu

log="$(dirname "$0")/../artifacts/require-tests.log"
mkdir -p "$(dirname "$log")"

status=0
cargo test "$@" >"$log" 2>&1 || status=$?
cat "$log"
if [ "$status" -ne 0 ]; then
  exit "$status"
fi

passed=$(awk '/^test result:/ {n += $4} END {print n + 0}' "$log")
if [ "$passed" -eq 0 ]; then
  echo "require_tests: no tests matched the filter: cargo test $*" >&2
  exit 1
fi
echo "require_tests: $passed passed"
