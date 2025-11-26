#!/usr/bin/env bash
set -euo pipefail

# build.sh — Build ALL xsfx variants using a single Linux Docker container
#
# Goal:
# - From ANY host (macOS/Linux/Windows with Docker), run a single Linux container
#   that cross-compiles:
#     - Linux (gnu+musl) x86_64/aarch64
#     - macOS (Darwin) x86_64/aarch64 via osxcross
#     - Windows (GNU) x86_64
#   and produces packers for each into ./dist.
# - Each produced packer embeds the same full multi-stub catalog by reusing a
#   prebuilt stub directory that we first generate inside the container.
# - This script will also be used later on CI (Linux runners).
#
# Notes/limits:
# - Windows ARM64 MSVC and Windows x64 MSVC cannot be built on Linux without
#   proprietary MSVC toolchains. Those stubs are skipped. Windows x64 GNU is built.
# - Building osxcross requires downloading a public macOS SDK tarball.
# - Host stays clean; all toolchains live inside a transient container.

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
PROJECT_DIR="$SCRIPT_DIR"
DIST_DIR="$PROJECT_DIR/dist"
mkdir -p "$DIST_DIR"

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker is required for this build. Please install Docker and retry." >&2
  exit 1
fi

echo "Building xsfx packers inside a Linux Docker container into $DIST_DIR"

# Use amd64 container to maximize toolchain availability (works on Apple Silicon via emulation)
DOCKER_PLATFORM="linux/amd64"

# Use our prebuilt toolchain image if available; otherwise build it now from Dockerfile
IMAGE_TAG="${XSFX_IMAGE_TAG:-xsfx-build}"

# Build the image once (and reuse on subsequent runs)
if ! docker image inspect "$IMAGE_TAG" >/dev/null 2>&1; then
  echo "Docker image '$IMAGE_TAG' not found. Building it now from Dockerfile..."
  # Allow overriding SDK URL via XSFX_MAC_SDK_URL
  BUILD_ARG_SDK=""
  if [ -n "${XSFX_MAC_SDK_URL:-}" ]; then
    BUILD_ARG_SDK="--build-arg MAC_SDK_URL=${XSFX_MAC_SDK_URL}"
  fi
  docker build --platform "$DOCKER_PLATFORM" -t "$IMAGE_TAG" $BUILD_ARG_SDK .
fi

# Pass through a cache volume for cargo if available to speed up builds between runs
CARGO_VOLUME="xsfx_cargo_cache"
docker volume inspect "$CARGO_VOLUME" >/dev/null 2>&1 || docker volume create "$CARGO_VOLUME" >/dev/null

# Build docker command as string for debugging
DOCKER_CMD="docker run --rm"
DOCKER_CMD+=" --platform $DOCKER_PLATFORM"
DOCKER_CMD+=" -v \"$PROJECT_DIR\":/project"
DOCKER_CMD+=" -v $CARGO_VOLUME:/usr/local/cargo/registry"
DOCKER_CMD+=" -w /project"
DOCKER_CMD+=" -e RUSTUP_HOME=/usr/local/rustup"
DOCKER_CMD+=" -e CARGO_HOME=/usr/local/cargo"
DOCKER_CMD+=" -e PROJECT_DIR=/project"
DOCKER_CMD+=" -e XSFX_TARGETS=${XSFX_TARGETS:-all}"
if [ -n "${ALL_STUBS:-}" ]; then
  DOCKER_CMD+=" -e ALL_STUBS=\"$ALL_STUBS\""
fi
if [ -n "${PACKER_TARGETS:-}" ]; then
  DOCKER_CMD+=" -e PACKER_TARGETS=\"$PACKER_TARGETS\""
fi
DOCKER_CMD+=" $IMAGE_TAG"

echo "Debug: Docker command to execute:"
echo "$DOCKER_CMD"
echo ""

# Run container and let the image entrypoint perform the build
# Disable Git Bash path conversion for Docker arguments
MSYS_NO_PATHCONV=1 eval "$DOCKER_CMD"

echo "Build finished. See ./dist for outputs."
