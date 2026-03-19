# TASK-004 — Add stdin/stdout pipe support to packer

**Status:** `DONE`
**Created:** 2026-03-25
**Updated:** 2026-03-25

## Raw Request

> task: let packer support stdin / stdout for pipeing

## Refined Description

**Scope:** Add `-` convention for stdin (payload input) and stdout (SFX output) to the packer CLI.
**Non-goals:** Streaming compression (payload is fully buffered before compression). Stub pipe support (stub reads its own executable, not stdin).
**Impacted UCs:** UC-001 (packing workflow extended with pipe I/O)
**Impacted BR/WF:** WF-001 (packing workflow)
**Dependencies:** None
**Risks / Open Questions:** None

## Estimation

**Level:** `LOW`
**Justification:** Single file change (packer.rs), well-scoped, clear acceptance criteria. Touches 1 source file + 2 doc files.

## Speck Reference

N/A — estimated LOW, straightforward feature with clear scope.
