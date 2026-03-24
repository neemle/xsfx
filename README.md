# xsfx

[![CI](https://github.com/neemle/xsfx/actions/workflows/ci.yml/badge.svg)](https://github.com/neemle/xsfx/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/neemle/xsfx)](https://github.com/neemle/xsfx/releases)

Self-extracting executable packer written in Rust. Compresses a payload binary with LZMA/XZ and bundles it with a per-platform stub that decompresses and executes it entirely in memory at runtime. No temporary files are written on any platform.

Does not modify PE headers, so packed .NET assemblies and other header-sensitive executables remain valid.

## Supported Targets

Each packer binary embeds stubs for all targets available at build time. The target is selected at pack time via `--target`.

| Target | Arch | Execution Method |
|---|---|---|
| `x86_64-unknown-linux-musl` | x64 | `memfd_create` + `execveat` |
| `aarch64-unknown-linux-musl` | ARM64 | `memfd_create` + `execveat` |
| `x86_64-apple-darwin` | x64 | `NSCreateObjectFileImageFromMemory` |
| `aarch64-apple-darwin` | ARM64 | `NSCreateObjectFileImageFromMemory` |
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

### Pipe Support

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

## Install

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/neemle/xsfx/releases).

```bash
# Linux / macOS
curl -sSfL https://github.com/neemle/xsfx/releases/latest/download/xsfx-x86_64-unknown-linux-gnu.tar.gz \
    | tar xzf - -C /usr/local/bin
```

### Build from Source

```bash
cargo build --release --bin xsfx --features native-compress
```

Without native liblzma (pure-Rust compression, lower ratio, no C compiler needed):

```bash
cargo build --release --bin xsfx --no-default-features
```

### Cross-build All Targets (Docker)

```bash
./build.sh
```

## Testing

```bash
# Docker (recommended)
docker compose run --build --rm test

# Or via CI script
./scripts/ci.sh

# Native
XSFX_SKIP_STUB_BUILD=1 cargo test --lib --test integration
```

100 tests including 59 security/adversarial tests. Coverage enforced at 100% (lines + functions) on library code.

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

## Compression

| Mode | Build Flag | Details |
|---|---|---|
| Ultra (default) | `--features native-compress` | `liblzma` via `xz2` (static) -- LZMA2 preset 9 extreme, 64 MiB dict, BinaryTree4 |
| Pure Rust | `--no-default-features` | `lzma-rs` encoder -- standard XZ settings |

The stub always uses the pure-Rust `lzma-rs` decoder regardless of packer compression mode.

## Documentation

- [`docs/spec.md`](docs/spec.md) -- Business specification
- [`docs/development-manual.md`](docs/development-manual.md) -- Developer guide
- [`docs/installation-manual.md`](docs/installation-manual.md) -- Installation
- [`docs/configuration-manual.md`](docs/configuration-manual.md) -- Configuration

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

To report a vulnerability, see [SECURITY.md](.github/SECURITY.md).

## License

[MIT](LICENSE)
