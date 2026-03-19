#!/usr/bin/env bash
# Unified CI simulation — rules §8.6 compliance
# Runs the full fail-fast pipeline locally via Docker.
set -euo pipefail

echo "=== xsfx CI simulation ==="
echo "Stage 1: fmt + clippy + coverage (100% lines/functions/regions) + audit"
docker compose run --build --rm test

echo ""
echo "=== CI simulation passed ==="
