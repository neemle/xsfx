#!/usr/bin/env bash
set -euo pipefail

# xsfx-entrypoint — runs inside the xsfx-build container
# Builds a catalog of prebuilt stubs and then builds packers that embed them.

echo "Container ready. Using pre-installed toolchains from image."

PROJECT_DIR=${PROJECT_DIR:-/project}
BUILD_DIR="$PROJECT_DIR/.build"
STUBS_DIR="$BUILD_DIR/stubs"
DIST_DIR="$PROJECT_DIR/dist"

mkdir -p "$DIST_DIR" "$STUBS_DIR"

# Allow callers to override the stub/packer target sets via env.
read -r -a ALL_STUBS <<< "${ALL_STUBS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-pc-windows-msvc}"
read -r -a PACKER_TARGETS <<< "${PACKER_TARGETS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu}"

echo "Building prebuilt stubs for all possible targets..."
built_any=0
for t in "${ALL_STUBS[@]}"; do
  echo "-- Building stub for $t"
  if cargo build --release --bin stub --target "$t"; then
    built_any=1
    mkdir -p "$STUBS_DIR/$t"
    if [[ "$t" == *windows* ]]; then
      cp -f "target/$t/release/stub.exe" "$STUBS_DIR/$t/stub.exe" || true
    else
      cp -f "target/$t/release/stub" "$STUBS_DIR/$t/stub" || true
    fi
  else
    echo "   Skipping $t (build failed or unsupported in Linux container)."
  fi
done

if [ "$built_any" -ne 1 ]; then
  echo "No stubs built; aborting." >&2
  exit 1
fi

echo "Building packers for selected output targets using the prebuilt stubs..."
export XSFX_TARGETS=${XSFX_TARGETS:-all}
export XSFX_PREBUILT_STUBS_DIR="$STUBS_DIR"

for t in "${PACKER_TARGETS[@]}"; do
  echo "==> Building xsfx for $t (embedding all available prebuilt stubs)"
  cargo build --release --bin xsfx --target "$t" --features native-compress
  if [[ "$t" == *windows* ]]; then
    cp -f "target/$t/release/xsfx.exe" "$DIST_DIR/xsfx-$t.exe"
  else
    cp -f "target/$t/release/xsfx" "$DIST_DIR/xsfx-$t"
  fi
done

echo "Done inside container. Artifacts in $DIST_DIR:"
ls -l "$DIST_DIR" || true
