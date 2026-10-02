#!/usr/bin/env bash
# Reproducible Release Build (security roadmap §4)
#
# Goal: two independent builds of the same commit produce byte-identical
# artifacts, so released binaries are verifiably built from this source.
#
# Rules for reproducibility:
#   1. EXACT toolchain (see src-tauri/rust-toolchain.toml — rustup enforces).
#   2. Locked dependencies (--locked; Cargo.lock is committed).
#   3. No absolute paths embedded: build from the same relative layout.
#   4. Fixed SOURCE_DATE_EPOCH so timestamps embed deterministically.
#   5. No local env leakage: clean environment except what's set below.
#
# Verify two builds:
#   ./scripts/build-release.sh out-a && ./scripts/build-release.sh out-b
#   diff <(cd out-a && sha256sum *) <(cd out-b && sha256sum *)
#
# NOTE: the reproducibility claim is still UNVERIFIED. Running this twice on two
# machines has not been done, and `docs/SECURITY-HARDENING.md` marks that as
# outstanding. Do not describe this script as proof of anything until it is.

set -euo pipefail
cd "$(dirname "$0")/.."

OUT_DIR="${1:?usage: build-release.sh <output-dir>}"
mkdir -p "$OUT_DIR"

export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
export CARGO_TERM_COLOR=never
# Deliberately NOT set:
#   RUSTFLAGS="-C codegen-units=1"
# RUSTFLAGS *replaces* the profile's rustflags rather than adding to them, so
# exporting it here silently discarded `strip`, `lto`, `opt-level = "z"` and
# `overflow-checks`. `codegen-units = 1` now comes from `[profile.release]` in
# the workspace-root Cargo.toml, which is the only place Cargo reads a profile
# from — see the comment there.
unset RUSTC_WRAPPER CARGO_INCREMENTAL 2>/dev/null || true

echo "==> Toolchain (pinned via src-tauri/rust-toolchain.toml)"
rustc --version

# `tauri build`, not `cargo build`. `cargo build` compiles the binary and stops
# there: it never runs `beforeBuildCommand`, so `dist/` may not exist, and it
# never produces `target/release/bundle/`. The previous version of this script
# ran `cargo build` and then `find`ed that directory — which matched nothing,
# and because the trailing `cp` was `|| true` the run still exited 0. It
# cheerfully reported success while collecting zero installers.
echo "==> Building bundles (locked deps, release profile, beforeBuildCommand)"
pnpm --dir . exec tauri build --bundles "${TAURI_BUNDLES:-deb,rpm,appimage,msi,nsis,app}"

echo "==> Collecting artifacts"
BUNDLE_DIR="src-tauri/target/release/bundle"
if [ ! -d "$BUNDLE_DIR" ]; then
  echo "FATAL: $BUNDLE_DIR does not exist. The build produced no bundles." >&2
  exit 1
fi

ARTIFACTS=0
while IFS= read -r -d '' f; do
  cp "$f" "$OUT_DIR/"
  ARTIFACTS=$((ARTIFACTS + 1))
done < <(find "$BUNDLE_DIR" -type f \
  \( -name '*.exe' -o -name '*.msi' -o -name '*.AppImage' \
     -o -name '*.deb' -o -name '*.rpm' -o -name '*.dmg' -o -name '*.app' \) \
  -print0)

if [ "$ARTIFACTS" -eq 0 ]; then
  echo "FATAL: no installer artifacts found under $BUNDLE_DIR." >&2
  echo "Refusing to report success with an empty output directory." >&2
  exit 1
fi
echo "==> Collected $ARTIFACTS artifact(s)"

echo "==> Hashes"
# Only the installers. Hashing `*` would also pick up `.sig` files if signing
# ran first, mixing two kinds of artifact into one hash list.
(cd "$OUT_DIR" && sha256sum *.* 2>/dev/null || sha256sum *)
echo "Done. Publish hashes alongside the release and sign them (see docs/SIGNED-UPDATES.md)."