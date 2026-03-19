# xsfx — Development Manual

## 1. Prerequisites

- **Docker** (for containerized testing and cross-compilation)
- **Rust 1.76+** (for native development; nightly required for stub minification)
- **C compiler** (gcc/clang for native builds with `native-compress` feature)
- **musl-tools** (for Linux musl target builds)

## 2. Repository Structure

```
xsfx/
├── src/
│   ├── lib.rs              # Library re-exports
│   ├── common.rs           # Trailer struct, magic constants
│   ├── compress.rs         # LZMA/XZ compression (packer)
│   ├── decompress.rs       # LZMA/XZ decompression (stub)
│   ├── pe_loader.rs        # Windows PE in-memory loader
│   ├── macho_loader.rs     # macOS Mach-O in-memory loader
│   └── bin/
│       ├── packer.rs       # CLI packer entry point
│       └── stub.rs         # SFX runtime (self-extract/execute)
├── tests/
│   └── integration.rs      # Integration tests
├── docs/                   # Documentation (this folder)
├── scripts/
│   └── xsfx-entrypoint.sh  # Docker cross-build entrypoint
├── .github/workflows/      # CI/CD
│   ├── ci.yml              # Test + build pipeline
│   └── release.yml         # Release pipeline
├── Cargo.toml              # Rust package manifest
├── Cargo.lock              # Pinned dependency versions
├── Dockerfile              # Multi-stage: test + cross
├── docker-compose.yml      # Docker Compose for testing
├── build.rs                # Build script (stub catalog generation)
├── build.sh                # Cross-compilation orchestration
├── .env                    # Safe local defaults
├── .dockerignore
└── .gitignore
```

## 3. Running Tests

### Docker (recommended)

```bash
docker compose run --build test
```

Or via the unified CI script:

```bash
./scripts/ci.sh
```

This runs the full fail-fast pipeline:
1. `cargo fmt --all -- --check` — format check
2. `cargo clippy --lib --test integration -- -D warnings` — linting
3. `cargo llvm-cov` — coverage (100% lines, functions, regions)
4. `cargo audit` — dependency vulnerability scan

### Native (fallback)

```bash
XSFX_SKIP_STUB_BUILD=1 cargo test --lib --test integration
```

`XSFX_SKIP_STUB_BUILD=1` skips stub compilation (requires nightly + cross toolchains).

## 4. Building

### Native Single-Target Build

```bash
cargo build --release --bin xsfx --features native-compress
```

This builds a packer that embeds only the host-platform stub.

### Cross-Compilation (All 9 Targets)

```bash
./build.sh
```

To build a subset:

```bash
PACKER_TARGETS="x86_64-unknown-linux-gnu" ./build.sh
```

The build script:
1. Builds/caches the `xsfx-build` Docker image (cross-compilation stage)
2. Creates a `xsfx_cargo_cache` Docker volume for dependency caching
3. Runs `scripts/xsfx-entrypoint.sh` inside the container
4. Outputs packaged binaries to `./dist/`

### Build Pipeline (inside Docker)

**Phase 1 — Build optimized stubs:**
- Uses nightly Rust with `-Z build-std=std,panic_abort`
- Applies `-Cpanic=immediate-abort` for minimal std footprint
- Post-processing:
  - Non-musl targets: UPX `--best --lzma` compression
  - Musl targets: `xstrip` ELF dead-code removal (UPX incompatible due to AT_BASE)

**Phase 2 — Build packers with prebuilt stubs:**
- Uses stable Rust with `native-compress` feature
- Sets `XSFX_PREBUILT_STUBS_DIR` to embed pre-optimized stubs
- Builds one packer per target platform

## 5. Platform-Specific Execution Strategies

| Platform | Method | Details |
|----------|--------|---------|
| Linux | `memfd_create` + `execveat` | Anonymous in-memory fd, process replacement via `AT_EMPTY_PATH` |
| Windows | In-process PE loader | Parse PE32+, VirtualAlloc, map sections, fix relocations, resolve imports |
| macOS | `NSCreateObjectFileImageFromMemory` | Patch MH_EXECUTE→MH_BUNDLE, link module, call `_main` |

## 6. Test Structure

Tests are organized as:
- **Unit tests:** embedded in each source module (`common.rs`, `compress.rs`, `decompress.rs`, `pe_loader.rs`, `macho_loader.rs`)
- **Integration tests:** `tests/integration.rs` (SFX format assembly, roundtrip, edge cases)
- **Security tests:** labeled `test_sec_ucXXX_*` covering adversarial inputs, memory leaks, corruption, boundary values

Coverage is enforced at 100% for lines, functions, and regions (excluding `bin/` targets).

## 7. Two-Stage SFX (Proof of Concept)

Located in `.dev-temp/xstrip-test/`. A two-stage format for musl targets:
- **stage0:** ~10 KB nostd loader with custom RFC 1951 inflate and raw x86_64 syscalls
- **stage1:** standard SFX (stub + xz(payload) + trailer)
- Achieves ~40% size reduction by deflate-compressing the 96 KB musl stub through a 10 KB loader

## 8. Adding a New Target

1. Add the target triple to `build.rs` `ALL_TARGETS` array
2. Configure cross-linker in `Dockerfile` (cross stage) and cargo config
3. Add platform-specific execution in `src/bin/stub.rs` (behind `#[cfg(target_os)]`)
4. Update `scripts/xsfx-entrypoint.sh` with stub-specific flags and post-processing
5. Update CI workflows if needed
6. Add tests for the new execution path
