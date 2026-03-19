# TASK-003 — Full Documentation Regeneration

**Status:** `DONE`
**Created:** 2026-03-25
**Updated:** 2026-03-25

## Raw Request

> now remove the docs folder and completely regenerate it

## Refined Description

**Scope:** Delete all existing documentation under `docs/` and regenerate from scratch by deriving behavior and architecture from the actual codebase (per rules v0.5.0 §4.1).
**Non-goals:** Code changes. UI/API documentation (N/A — CLI tool). Inventing business intent not supported by code.
**Impacted UCs:** UC-001, UC-002, UC-003 (documentation only, no behavioral changes)
**Impacted BR/WF:** BR-001..BR-015, WF-001..WF-002 (documentation only)
**Dependencies:** None
**Risks / Open Questions:** None

## Estimation

**Level:** `LOW`
**Justification:** Documentation-only task. All information derivable from existing source code, build scripts, and CI config. No code changes required.

## Speck Reference

N/A — documentation-only task, no code changes, no Speck needed.
