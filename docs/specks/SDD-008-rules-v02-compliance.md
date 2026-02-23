# SDD-008 — Rules v0.2.0 Compliance

**Impacted UCs:** None (infrastructure-only)
**Impacted BR/WF:** None

## Scope / Non-goals

**Scope:** Bring codebase into compliance with `docs/rules.md` v0.2.0.

- Create `docs/backlog/` infrastructure (new requirement from v0.2.0 section 12)
- Pin all dependencies to specific patch versions (section 9.2)
- Pin Docker base images to specific minor/patch tags (section 8.3)
- Refactor functions exceeding 30-line limit (section 5.2)
- Document security test naming adaptation for Rust (section 6.4)

**Non-goals:**

- Retroactive backlog files for pre-v0.2.0 specks
- Functional or behavioral changes

## Acceptance Criteria

- AC-1: `docs/backlog/` directory exists with `TASK-001-rules-v02-compliance.md`
- AC-2: All Cargo.toml dependencies pinned to exact patch versions
- AC-3: Docker base images pinned to `rust:1.93.1-slim-bookworm` and `rust:1.93.1-bookworm`
- AC-4: All functions comply with the 30 non-empty, non-comment line limit
- AC-5: Security test naming documented as Rust-specific adaptation (`test_sec_*` prefix)

## Security Acceptance Criteria (mandatory)

- SEC-1: Refactoring is behavior-preserving; no security regression in PE loader validation
- SEC-2: No test coverage loss from function extraction

## Failure Modes / Error Mapping

N/A (infrastructure-only, no runtime behavior changes)

## Rust-Specific Adaptations

### Security Test Naming (section 6.4)

Rules v0.2.0 requires `[SEC]` bracket labels in test titles. Rust function
identifiers cannot contain brackets (`[`, `]`), making the literal `[SEC]`
syntax impossible. The codebase uses `test_sec_*` prefix as the idiomatic
Rust equivalent. All 34 security tests follow this convention consistently.

## Test Matrix (mandatory)

| AC    | Unit | Integration |
|-------|------|-------------|
| AC-1  | N/A  | N/A         |
| AC-2  | N/A  | N/A         |
| AC-3  | N/A  | N/A         |
| AC-4  | N/A  | N/A         |
| AC-5  | N/A  | N/A         |
| SEC-1 | N/A  | N/A         |
| SEC-2 | N/A  | N/A         |

Notes: All ACs are infrastructure/documentation changes verified by code review.
No new tests required — existing tests validate behavior preservation.
