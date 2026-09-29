#!/usr/bin/env bash
# Copy the four live modules in, then run their tests.
#
# These modules have no GTK/Tauri dependency, so unlike the typecheck harness
# this actually *executes* their #[cfg(test)] suites. Copying (never
# symlinking — see tools/typecheck-harness/README.md) means the tests run
# against the real source, so a test cannot pass by testing a stale copy.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC="$REPO/src-tauri/src"
HERE="$(cd "$(dirname "$0")" && pwd)"

for m in crypto group protocol secure_key storage; do
  if [ -L "$HERE/src/$m.rs" ]; then
    echo "FATAL: $HERE/src/$m.rs is a symlink — refusing." >&2
    echo "A harness wrote through a symlink here before and truncated the live tree." >&2
    exit 1
  fi
done

for m in crypto group protocol secure_key storage; do
  cp "$SRC/$m.rs" "$HERE/src/$m.rs"
done
cd "$HERE"
cargo test --offline --lib "$@"
