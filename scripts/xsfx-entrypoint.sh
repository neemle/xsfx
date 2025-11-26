#!/usr/bin/env bash
set -euo pipefail

# xsfx-entrypoint — runs inside the xsfx-build container
# Builds a catalog of prebuilt stubs and then builds packers that embed them.

echo "Container ready. Using pre-installed toolchains from image."

# Ensure all required Rust targets are installed
echo "Verifying Rust targets..."
rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
rustup target add x86_64-unknown-linux-musl 2>/dev/null || true
rustup target add aarch64-unknown-linux-musl 2>/dev/null || true
rustup target add x86_64-apple-darwin 2>/dev/null || true
rustup target add aarch64-apple-darwin 2>/dev/null || true
rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
rustup target add x86_64-pc-windows-msvc 2>/dev/null || true
rustup target add aarch64-pc-windows-msvc 2>/dev/null || true

PROJECT_DIR=${PROJECT_DIR:-/project}
BUILD_DIR="$PROJECT_DIR/.build"
STUBS_DIR="$BUILD_DIR/stubs"
DIST_DIR="$PROJECT_DIR/dist"

# Clean up stub directory to track what actually builds
echo "Cleaning stub directory to track fresh builds..."
rm -rf "$STUBS_DIR"
mkdir -p "$DIST_DIR" "$STUBS_DIR"

# Allow callers to override the stub/packer target sets via env.
read -r -a ALL_STUBS <<< "${ALL_STUBS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-pc-windows-msvc}"
read -r -a PACKER_TARGETS <<< "${PACKER_TARGETS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu}"

echo "Building prebuilt stubs for all possible targets..."
built_any=0
successful_stubs=()
failed_stubs=()

for t in "${ALL_STUBS[@]}"; do
  echo "-- Building stub for $t"
  if cargo build --release --bin stub --target "$t"; then
    built_any=1
    mkdir -p "$STUBS_DIR/$t"
    if [[ "$t" == *windows* ]]; then
      cp -f "target/$t/release/stub.exe" "$STUBS_DIR/$t/stub.exe" || true
      echo "   ✓ Successfully built and copied stub.exe for $t"
    else
      cp -f "target/$t/release/stub" "$STUBS_DIR/$t/stub" || true
      echo "   ✓ Successfully built and copied stub for $t"
    fi
    successful_stubs+=("$t")
  else
    echo "   ✗ Failed to build stub for $t (build failed or unsupported in Linux container)"
    failed_stubs+=("$t")
  fi
done

echo ""
echo "=== STUB BUILD SUMMARY ==="
echo "Successful stubs (${#successful_stubs[@]}): ${successful_stubs[*]}"
echo "Failed stubs (${#failed_stubs[@]}): ${failed_stubs[*]}"
echo "========================="
echo ""

if [ "$built_any" -ne 1 ]; then
  echo "No stubs built; aborting." >&2
  exit 1
fi

echo "Building packers for selected output targets using the prebuilt stubs..."
# Use only the successfully built stubs
export XSFX_TARGETS=$(IFS=,; echo "${successful_stubs[*]}")
export XSFX_PREBUILT_STUBS_DIR="$STUBS_DIR"
echo "Using stub targets: $XSFX_TARGETS"
echo ""

for t in "${PACKER_TARGETS[@]}"; do
  echo "==> Building xsfx for $t (embedding ${#successful_stubs[@]} available prebuilt stubs)"
  if cargo build --release --bin xsfx --target "$t" --features native-compress; then
    if [[ "$t" == *windows* ]]; then
      cp -f "target/$t/release/xsfx.exe" "$DIST_DIR/xsfx-$t.exe"
      echo "   ✓ Successfully built packer xsfx-$t.exe"
    else
      cp -f "target/$t/release/xsfx" "$DIST_DIR/xsfx-$t"
      echo "   ✓ Successfully built packer xsfx-$t"
    fi
  else
    echo "   ✗ Failed to build packer for $t"
  fi
done

echo "Done inside container. Artifacts in $DIST_DIR:"
ls -l "$DIST_DIR" || true
