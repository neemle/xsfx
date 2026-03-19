# xsfx

Self-extracting executable packer written in Rust. Compresses a payload binary with LZMA/XZ and bundles it with a per-platform stub that decompresses and executes it entirely in memory at runtime. No temporary files are written on any platform.

Does not modify PE headers, so packed .NET assemblies and other header-sensitive executables remain valid.

## Supported Targets

Each packer binary embeds stubs for all targets available at build time. The target is selected at pack time via `--target`.

| Target | Arch | Execution Method |
|---|---|---|
| `x86_64-unknown-linux-gnu` | x64 | `memfd_create` + `execveat` |
| `aarch64-unknown-linux-gnu` | ARM64 | `memfd_create` + `execveat` |
| `x86_64-unknown-linux-musl` | x64 | `memfd_create` + `execveat` |
| `aarch64-unknown-linux-musl` | ARM64 | `memfd_create` + `execveat` |
| `x86_64-apple-darwin` | x64 | `NSCreateObjectFileImageFromMemory` |
| `aarch64-apple-darwin` | ARM64 | `NSCreateObjectFileImageFromMemory` |
| `x86_64-pc-windows-gnu` | x64 | In-process PE loader |
| `x86_64-pc-windows-msvc` | x64 | In-process PE loader |
| `aarch64-pc-windows-msvc` | ARM64 | In-process PE loader |

## Usage

```
xsfx <input> <output> [--target <triple>]
```

- `input` -- payload binary to pack (use `-` to read from stdin)
- `output` -- output path for the self-extracting executable (use `-` to write to stdout)
- `--target` -- target triple (defaults to the packer's host platform)

Running the packer without arguments lists available targets.

All CLI arguments passed to the SFX at runtime are forwarded to the payload.

### Pipe support

```bash
# Read payload from stdin
cat myapp | xsfx - myapp-sfx

# Write SFX to stdout
xsfx myapp - > myapp-sfx

# Full pipe
cat myapp | xsfx - - > myapp-sfx

# Pipe over SSH
xsfx myapp - --target x86_64-unknown-linux-gnu | ssh server 'cat > myapp-sfx && chmod +x myapp-sfx'
```

## Build

### Cross-build (Docker)

```bash
./build.sh
```

Builds packers for all 9 targets into `./dist/`. Requires Docker. The toolchain image is built on first run and cached.

To build a subset:

```bash
PACKER_TARGETS="x86_64-unknown-linux-gnu x86_64-pc-windows-msvc" ./build.sh
```

### Native build (single target)

```bash
cargo build --release --bin xsfx --features native-compress
```

Builds a packer for the host platform with ultra compression. The host stub is compiled by `build.rs`.

### Without native liblzma

```bash
cargo build --release --bin xsfx --no-default-features
```

Uses pure-Rust lzma-rs for compression (lower ratio, no C compiler needed).

## Testing

```bash
# Docker (recommended)
docker compose run --build test

# Or via CI script
./scripts/ci.sh

# Native
XSFX_SKIP_STUB_BUILD=1 cargo test --lib --test integration
```

Tests enforce 100% coverage (lines, functions, regions) on library code. 94+ tests including 52+ security/adversarial tests covering corruption, boundary values, memory leaks, and oversized payloads.

## Binary Format

```
+------------------------+
| Stub                   |  per-platform loader (<100 KB)
+------------------------+
| Compressed payload     |  LZMA/XZ stream
+------------------------+
| Trailer (16 bytes)     |  payload_len (u64 LE) + magic (u64 LE)
+------------------------+
```

The stub reads the 16-byte trailer from the end of its own executable, locates and decompresses the payload, then executes it in memory using the platform-specific method listed above.

## Compression

| Mode | Build Flag | Details |
|---|---|---|
| Ultra (default) | `--features native-compress` | `liblzma` via `xz2` (static) -- LZMA2 preset 9 extreme, 64 MiB dict, BinaryTree4 |
| Pure Rust | `--no-default-features` | `lzma-rs` encoder -- standard XZ settings |

The stub always uses the pure-Rust `lzma-rs` decoder regardless of packer compression mode.

## Documentation

Full documentation lives in `docs/`:

- [`docs/spec.md`](docs/spec.md) -- Business specification (use cases, business rules, workflows)
- [`docs/development-manual.md`](docs/development-manual.md) -- Developer guide
- [`docs/installation-manual.md`](docs/installation-manual.md) -- End-user installation
- [`docs/configuration-manual.md`](docs/configuration-manual.md) -- Build and runtime configuration

## License

MIT
