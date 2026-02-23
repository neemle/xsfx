# Development Manual

## Prerequisites

- Docker (required for running tests)
- Rust stable toolchain (1.76+, only for native builds without Docker)
- C compiler (for building vendored liblzma from source; included in Docker images)
- `musl-tools` (Linux, for building the static stub)

## Repository Structure

```text
src/
  lib.rs            Re-exports: common, compress, decompress, pe_loader, macho_loader
  common.rs         Trailer struct, MAGIC, TRAILER_SIZE
  compress.rs       compress_lzma (XZ compression)
  decompress.rs     decompress_payload (XZ decompression)
  pe_loader.rs      Windows PE in-memory loading (compiled only on Windows)
  macho_loader.rs   macOS Mach-O in-memory loading (compiled only on macOS)
  bin/
    packer.rs       CLI entry point for packing
    stub.rs         SFX runtime (self-extract and execute)
tests/
  integration.rs    Integration tests (round-trip, format, security)
docs/
  spec.md           Business specification
  rules.md          Development rules
  specks/           Speck-driven development task files
```

## Building

### Build the stub (must be built first)

#### Linux (static via musl -- recommended)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --bin stub --target x86_64-unknown-linux-musl
```

#### Windows (static via crt-static)

```bash
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --bin stub --target x86_64-pc-windows-msvc
```

#### macOS

```bash
cargo build --release --bin stub --target aarch64-apple-darwin
```

### Build the packer (requires stub path)

```bash
export XSFX_STUB_PATH="$(pwd)/target/x86_64-unknown-linux-musl/release/stub"
cargo build --release --bin xsfx
```

Ultra compression (liblzma) is enabled by default via vendored static linking —
no system `liblzma-dev` needed. To build without it (pure-Rust fallback):

```bash
cargo build --release --bin xsfx --no-default-features
```

## Platform-Specific Execution Strategies

The stub uses platform-specific in-memory execution on all platforms:

| Platform | Strategy | Mechanism |
|----------|----------|-----------|
| Linux | memfd_create | Anonymous in-memory file via syscall, executed via execveat(AT_EMPTY_PATH) |
| Windows | In-process PE loading | Parse PE headers, VirtualAlloc, map sections, resolve imports, call entry point |
| macOS | NSObjectFileImage API | Patch MH_EXECUTE to MH_BUNDLE, load via NSCreateObjectFileImageFromMemory |

No temp files are used on any platform.

## Testing (Docker -- primary method)

All tests run inside Docker containers. No local Rust toolchain required.

### Run the full test suite (fmt + clippy + tests + coverage + audit)

```bash
docker compose run --build test
```

This single command executes, in fail-fast order:

1. `cargo fmt --all -- --check`
2. `cargo clippy --lib --tests -- -D warnings`
3. `cargo llvm-cov` with 100% line/function coverage enforcement
4. `cargo audit`

### Build the binary via Docker (includes static musl stub)

```bash
docker build --target build -t xsfx-build .
```

The Docker build compiles the stub with `x86_64-unknown-linux-musl` for a fully
static binary, then embeds it into the packer.

## Testing (native -- without Docker)

Requires local Rust toolchain and `cargo-llvm-cov`, `cargo-audit`.

```bash
cargo test --lib --tests
cargo llvm-cov --lib --tests
cargo fmt --all -- --check
cargo clippy --lib --tests -- -D warnings
cargo audit
```

## Two-Stage SFX (musl minification PoC)

The `.dev-temp/xstrip-test/` directory contains a proof-of-concept for a
two-stage SFX format that reduces musl SFX size by ~40%. See SDD-007 for
full details.

### Components

| Component | Path | Description |
|-----------|------|-------------|
| stage0 | `.dev-temp/xstrip-test/stage0/` | nostd loader (~10 KB), custom inflate, raw syscalls |
| assembler | `.dev-temp/xstrip-test/assembler/` | Test tool: assembles stage1 and two-stage SFX |
| Dockerfile | `.dev-temp/xstrip-test/Dockerfile` | Builds everything, runs functional tests |

### Running the PoC

From the `neemle/` parent directory (Docker context):

```bash
docker build \
  --build-arg EXTRA_CA_CERTS="$(cat ~/zscaler.crt)" \
  -f xsfx/.dev-temp/xstrip-test/Dockerfile \
  --target test -t xstrip-stage0-test .
```

This builds stage0 (nostd), the real stub (nightly+build-std), xstrip,
the assembler, and a hello-world payload. It then tests all four SFX
variants (original, xstrip'd, two-stage original, two-stage xstrip'd)
and prints a comparison report.

### Architecture

```text
Two-stage SFX layout:
[stage0 ~10KB] [deflate(stage1_sfx)] [trailer 24B]
                     |
                     v
              [stub ~96KB] [xz(payload)] [trailer 16B]
```

Stage0 reads itself, inflates stage1 into a memfd, execveat's it.
Stage1 (the normal stub) opens itself via `/proc/self/exe` (works
for memfds), decompresses the XZ payload, and execveat's the payload.

## Cross-Compilation

See `.github/workflows/ci.yml` for the full target matrix. Key targets:

| Target                        | OS      | Stub Target                     | Notes              |
|-------------------------------|---------|---------------------------------|--------------------|
| `x86_64-unknown-linux-gnu`    | Linux   | `x86_64-unknown-linux-musl`     | native-compress    |
| `aarch64-unknown-linux-gnu`   | Linux   | `aarch64-unknown-linux-musl`    | cross, native      |
| `x86_64-unknown-linux-musl`   | Linux   | `x86_64-unknown-linux-musl`     | static, pure Rust  |
| `aarch64-apple-darwin`        | macOS   | `aarch64-apple-darwin`          | native-compress    |
| `x86_64-apple-darwin`         | macOS   | `x86_64-apple-darwin`           | native-compress    |
| `x86_64-pc-windows-msvc`     | Windows | `x86_64-pc-windows-msvc`        | native-compress    |
| `aarch64-pc-windows-msvc`    | Windows | `aarch64-pc-windows-msvc`       | native-compress    |

Linux stubs always use musl for static linking. Windows stubs use `crt-static`.

### Post-Build Stub Processing

| Target | Tool | Effect |
|--------|------|--------|
| Non-musl | UPX `--best --lzma` | LZMA-compressed executable (~55% reduction) |
| `*-linux-musl` | xstrip `-i` | ELF dead-code removal (minimal on LTO builds) |

UPX is skipped for musl stubs (AT_BASE incompatibility). xstrip
([ratushnyi-labs/xstrip](https://github.com/ratushnyi-labs/xstrip)) is
applied instead for ELF-level analysis and dead code removal.

Cross-compilation requires the appropriate linker and target installed:

```bash
rustup target add <target>
```
