#!/usr/bin/env bash
set -euo pipefail

# xsfx-entrypoint — runs inside the xsfx-build container
# Builds optimized stubs (nightly + build-std + UPX) then packers embedding them.

echo "=== xsfx cross-build container ==="

PROJECT_DIR=${PROJECT_DIR:-/project}
BUILD_DIR="$PROJECT_DIR/.build"
STUBS_DIR="$BUILD_DIR/stubs"
DIST_DIR="$PROJECT_DIR/dist"

rm -rf "$STUBS_DIR"
mkdir -p "$DIST_DIR" "$STUBS_DIR"

# Stub targets (all platforms)
read -r -a ALL_STUBS <<< "${ALL_STUBS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-pc-windows-msvc}"

# Packer targets (platforms we can build the packer for from Linux)
read -r -a PACKER_TARGETS <<< "${PACKER_TARGETS:-x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-pc-windows-msvc}"

# ---------------------------------------------------------------------------
# Phase 1: Build stubs with nightly + build-std + UPX for minimal size
# ---------------------------------------------------------------------------
echo ""
echo "=== Phase 1: Building optimized stubs ==="

successful_stubs=()
failed_stubs=()

for t in "${ALL_STUBS[@]}"; do
  echo "-- Building stub for $t (nightly + build-std)"

  STUB_RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort"

  # Static linking flags (zig's musl toolchain links dynamically by default)
  case "$t" in
    *linux-musl*)   STUB_RUSTFLAGS="$STUB_RUSTFLAGS -C target-feature=+crt-static" ;;
    *windows-msvc*) STUB_RUSTFLAGS="$STUB_RUSTFLAGS -C target-feature=+crt-static" ;;
    *windows-gnu*)  STUB_RUSTFLAGS="$STUB_RUSTFLAGS -C target-feature=+crt-static" ;;
  esac

  if XSFX_SKIP_STUB_BUILD=1 RUSTFLAGS="$STUB_RUSTFLAGS" cargo +nightly build \
       -Z build-std=std,panic_abort \
       --release --bin stub --target "$t" 2>&1; then

    STUB_FILE="stub"
    [[ "$t" == *windows* ]] && STUB_FILE="stub.exe"
    STUB_PATH="target/$t/release/$STUB_FILE"

    # UPX compression — skip for musl targets (UPX in-process decompression
    # preserves stale AT_BASE in auxv, causing musl to exit 127)
    case "$t" in
      *linux-musl*)
        echo "   UPX skipped for $t (musl AT_BASE incompatibility)"
        ;;
      *)
        if upx --best --lzma "$STUB_PATH" 2>/dev/null; then
          echo "   UPX compressed $STUB_FILE"
        else
          echo "   UPX skipped for $t (unsupported format)"
        fi
        ;;
    esac

    mkdir -p "$STUBS_DIR/$t"
    cp -f "$STUB_PATH" "$STUBS_DIR/$t/$STUB_FILE"

    SIZE=$(stat -c%s "$STUBS_DIR/$t/$STUB_FILE" 2>/dev/null || stat -f%z "$STUBS_DIR/$t/$STUB_FILE" 2>/dev/null || echo "?")
    echo "   OK: $t ($SIZE bytes)"
    successful_stubs+=("$t")
  else
    echo "   FAIL: $t"
    failed_stubs+=("$t")
  fi
done

echo ""
echo "=== STUB SUMMARY ==="
echo "OK (${#successful_stubs[@]}): ${successful_stubs[*]}"
if [ ${#failed_stubs[@]} -gt 0 ]; then
  echo "FAIL (${#failed_stubs[@]}): ${failed_stubs[*]}"
fi
echo "===================="

if [ ${#successful_stubs[@]} -eq 0 ]; then
  echo "No stubs built; aborting." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Phase 2: Build packers with prebuilt stubs + native-compress
# ---------------------------------------------------------------------------
echo ""
echo "=== Phase 2: Building packers ==="

export XSFX_TARGETS=$(IFS=,; echo "${successful_stubs[*]}")
export XSFX_PREBUILT_STUBS_DIR="$STUBS_DIR"

for t in "${PACKER_TARGETS[@]}"; do
  echo "==> Building xsfx for $t (embedding ${#successful_stubs[@]} stubs)"

  if cargo build --release --bin xsfx --target "$t" --features native-compress 2>&1; then
    PACKER_FILE="xsfx"
    [[ "$t" == *windows* ]] && PACKER_FILE="xsfx.exe"

    cp -f "target/$t/release/$PACKER_FILE" "$DIST_DIR/xsfx-$t${PACKER_FILE#xsfx}"
    echo "   OK: xsfx-$t"
  else
    echo "   FAIL: xsfx for $t"
  fi
done

echo ""
echo "=== Build complete ==="
echo "Artifacts:"
ls -lh "$DIST_DIR" 2>/dev/null || echo "(none)"
