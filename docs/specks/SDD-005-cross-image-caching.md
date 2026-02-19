# SDD-005 — Cache cross-compilation Docker image in CI

**Impacted UCs:** None (CI infrastructure only)
**Impacted BR/WF:** None

## Problem

The cross-compilation Docker image takes 30–60+ minutes to build from scratch.
It installs osxcross, xwin, zig, nightly Rust with 9 targets, UPX, and all
cross-linkers. Every CI run rebuilds this image because GitHub Actions runners
start with a clean Docker state. During active development this dominates
build time.

## Scope

- Cache the cross-build Docker image in GitHub Container Registry (ghcr.io)
  using a content-based tag derived from `Dockerfile` + `scripts/xsfx-entrypoint.sh`
- Pull the cached image before `./build.sh` so it skips the Docker build
- Push after a cache miss so subsequent runs hit the cache
- Apply to both `ci.yml` (build job) and `release.yml` (build-and-release job)

## Non-goals

- Caching the test stage image (lightweight, rebuilds fast)
- Automated cleanup of old cached images (ghcr.io retention handles this)
- Changing the local `build.sh` workflow (already caches locally via
  `docker image inspect`)

## Acceptance Criteria

- AC-1: On cache hit, `./build.sh` skips the Docker image build entirely
- AC-2: On cache miss (first run or Dockerfile changed), image is built and
  pushed to ghcr.io for future runs
- AC-3: Cache key is derived from the content of `Dockerfile` and
  `scripts/xsfx-entrypoint.sh` — any change to either file invalidates
  the cache
- AC-4: Push failures (e.g., fork PRs with limited token scope) do not
  fail the build

## Security Acceptance Criteria (mandatory)

- SEC-1: Only `GITHUB_TOKEN` is used for registry auth (no external secrets)
- SEC-2: `packages: write` permission is scoped to the build job only, not
  the test job

## Failure Modes / Error Mapping

| Condition               | Behavior                                     |
|-------------------------|----------------------------------------------|
| ghcr.io pull fails      | Falls through to full Docker build            |
| ghcr.io push fails      | Build succeeds, next run rebuilds (no cache)  |
| Login fails (fork PR)   | Pull may fail, build proceeds normally        |

## Test Matrix (mandatory)

| AC    | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod API | Prod Fullstack |
|-------|------|-------------|----------|---------|----|---------------|----------------|
| AC-1  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| AC-2  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| AC-3  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| AC-4  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| SEC-1 | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| SEC-2 | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |

Notes:
- CI infrastructure change — verified by observing cache hit/miss in workflow logs.
