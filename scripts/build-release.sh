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
#   sha256sum out-a/* out-b/*

set -euo pipefail
cd "$(dirname "$0")/.."

OUT_DIR="${1:?usage: build-release.sh <output-dir>}"
mkdir -p "$OUT_DIR"

export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
export CARGO_TERM_COLOR=never
export RUSTFLAGS="-C codegen-units=1"
unset RUSTC_WRAPPER CARGO_INCREMENTAL 2>/dev/null || true

echo "==> Toolchain (pinned via src-tauri/rust-toolchain.toml)"
cd src-tauri
rustc --version

echo "==> Building (locked deps, release profile)"
cargo build --release --locked

echo "==> Collecting artifacts"
BUNDLE_DIR="target/release/bundle"
find "$BUNDLE_DIR" -type f \( -name '*.exe' -o -name '*.msi' -o -name '*.AppImage' -o -name '*.deb' -o -name '*.dmg' \) \
  -exec cp {} "$OUT_DIR/" \;
cp target/release/m2m "$OUT_DIR/" 2>/dev/null || cp target/release/m2m.exe "$OUT_DIR/" 2>/dev/null || true

echo "==> Hashes"
(cd "$OUT_DIR" && sha256sum *)
echo "Done. Publish hashes alongside the release and sign them (see docs/SIGNED-UPDATES.md)."
