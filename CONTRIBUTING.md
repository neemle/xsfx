# Contributing to xsfx

Contributions are welcome. This document describes how to contribute effectively.

## Development Setup

1. Install [Rust](https://rustup.rs/) (stable + nightly)
2. Install [Docker](https://docs.docker.com/get-docker/) (for testing)
3. Clone the repository and run tests:

```bash
git clone https://github.com/ratushnyi-labs/xsfx.git
cd xsfx
docker compose run --build --rm test
```

## Workflow

1. Fork the repository
2. Create a feature branch from `main`: `git checkout -b feat/my-feature`
3. Make your changes
4. Run tests: `docker compose run --build --rm test`
5. Commit with conventional commit messages
6. Open a pull request against `main`

## Code Style

- Max line width: **120 characters**
- No dead code, TODO comments, or commented-out logic
- No `any` types (TypeScript) or `dynamic` (C#) — Rust enforces this naturally
- `///` doc comments on all new public functions
- Follow existing naming conventions: `snake_case` functions, `CamelCase` types, `SCREAMING_SNAKE` constants

## Safety Rules

xsfx processes untrusted binary data. All contributions MUST follow these safety rules:

- **Bounds check** all reads from binary data — never index without validation
- **No `unwrap()`/`expect()`** on data from files or user input — use `?` or proper error handling
- **Checked arithmetic** for offset and size calculations — use `checked_add()`, `saturating_mul()`, etc.
- **No resource leaks** — close/dispose all file handles, streams, and allocated memory
- **Validate before acting** — verify magic numbers, sizes, and offsets before using them

## Testing Requirements

- All existing tests MUST pass before submitting a PR
- New features MUST include tests (positive + negative + security/adversarial)
- Security tests MUST be labeled with `test_sec_ucXXX_` prefix
- Coverage must remain at 100% for library code (lines + functions)

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation changes
- `ci:` — CI/CD changes
- `test:` — test additions/changes
- `refactor:` — code restructuring without behavior change
- `chore:` — maintenance tasks

## PR Checklist

Before submitting a pull request, verify:

- [ ] Tests pass (`docker compose run --build --rm test`)
- [ ] No `unsafe` code without clear justification and comment
- [ ] Safety checks in place (bounds, error handling)
- [ ] Doc comments on new public functions
- [ ] Conventional commit messages used
- [ ] No dead code, TODOs, or commented-out logic
- [ ] Line width ≤ 120 characters
