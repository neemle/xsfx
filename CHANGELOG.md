# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-03-25

### Added
- **Pipe support**: read payload from stdin and/or write SFX to stdout using `-`
- **User manuals** in 6 languages: English, Ukrainian, Spanish, French, Italian, Portuguese
- Parallel CI/CD pipeline — stub and packer builds run concurrently on native OS runners
- Self-compression step in CI (xsfx packs itself for all 6 targets)
- Release workflow on `v*` tags with automatic GitHub Release assets
- Open-source governance: CONTRIBUTING.md, SECURITY.md, CHANGELOG.md, issue/PR templates, .gitattributes

### Changed
- **Target matrix reduced from 9 to 6**: dropped `linux-gnu` (musl static is better for distribution) and `windows-gnu` (MSVC is the standard toolchain). Remaining: `x86_64/aarch64` for `linux-musl`, `apple-darwin`, `windows-msvc`
- CI migrated from sequential Docker cross-build (~120 min) to parallel native runners (~18 min)
- Security tests renamed to `test_sec_ucXXX_*` convention
- Refactored `parse_pe()` into `validate_coff_headers()` + `parse_optional_header()` for maintainability
- Documentation fully regenerated from codebase

### Fixed
- Repo URLs corrected from `neemle/xsfx` to `ratushnyi-labs/xsfx` across all files
- Windows CI packaging step failing due to `set -e` with `[[ ]]` short-circuit
- Code formatting aligned with `cargo fmt`

### Security
- 100 tests (77 unit + 23 integration), 59 security/adversarial
- Dependencies audited — no known CVEs

## [0.1.7] - 2026-02-23

### Added
- Vendored static liblzma for always-on ultra compression
- Security, adversarial, and memory leak tests across all modules
- `.dev-data/` and `.dev-temp/` directory conventions

### Changed
- Pinned all dependency versions
- Refactored oversized functions for rules compliance

## [0.1.0] - 2025-01-01

### Added
- Initial release
- LZMA/XZ compression with pure-Rust decompression in stub
- In-memory execution: `memfd_create` (Linux), PE loader (Windows), `NSObjectFileImage` (macOS)
- Multi-stub catalog embedding at build time
- Cross-compilation Docker build system
