# xsfx

[![CI](https://github.com/ratushnyi-labs/xsfx/actions/workflows/ci.yml/badge.svg)](https://github.com/ratushnyi-labs/xsfx/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/ratushnyi-labs/xsfx)](https://github.com/ratushnyi-labs/xsfx/releases)

Self-extracting executable packer. Compresses a payload binary with LZMA2 and bundles it with a platform-specific stub that decompresses and runs it entirely in memory — no temporary files on any platform. Compatible with .NET assemblies and other header-sensitive executables (PE headers are not modified).

## Supported Targets

| Target | Arch | In-Memory Execution |
|---|---|---|
| `x86_64-unknown-linux-musl` | x64 | `memfd_create` + `execveat` |
| `aarch64-unknown-linux-musl` | ARM64 | `memfd_create` + `execveat` |
| `x86_64-apple-darwin` | x64 | `NSCreateObjectFileImageFromMemory` |
| `aarch64-apple-darwin` | ARM64 | `NSCreateObjectFileImageFromMemory` |
| `x86_64-pc-windows-msvc` | x64 | In-process PE loader |
| `aarch64-pc-windows-msvc` | ARM64 | In-process PE loader |

Each packer binary embeds stubs for all 6 targets. The target is selected at pack time via `--target` (defaults to host platform).

## Usage

```
xsfx <input> <output> [--target <triple>]
```

- `input` — payload binary to pack (use `-` for stdin)
- `output` — output path for the self-extracting executable (use `-` for stdout)
- `--target` — target triple (run without arguments to list available targets)

Arguments passed to the SFX at runtime are forwarded to the payload.

### Pipe Support

```bash
# Read from stdin
cat myapp | xsfx - myapp-sfx

# Write to stdout
xsfx myapp - > myapp-sfx

# Full pipe
cat myapp | xsfx - - > myapp-sfx

# Pack and deploy over SSH
xsfx myapp - --target x86_64-unknown-linux-musl | ssh server 'cat > myapp && chmod +x myapp'
```

## Install

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/ratushnyi-labs/xsfx/releases).

```bash
# Linux x64
curl -sSfL https://github.com/ratushnyi-labs/xsfx/releases/latest/download/xsfx-x86_64-unknown-linux-musl.tar.gz \
    | tar xzf - -C /usr/local/bin
```

### Build from Source

```bash
cargo build --release --bin xsfx
```

By default, compression uses `liblzma` via the `native-compress` feature. To build with pure-Rust compression (lower ratio, no C toolchain needed):

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

# Native
XSFX_SKIP_STUB_BUILD=1 cargo test --lib --test integration
```

100 tests (77 unit + 23 integration), 59 security/adversarial.

## Binary Format

```
+------------------------+
| Stub                   |  platform-specific loader (<100 KB)
+------------------------+
| Compressed payload     |  LZMA2/XZ stream
+------------------------+
| Trailer (16 bytes)     |  payload_len (u64 LE) + magic (u64 LE)
+------------------------+
```

## Documentation

- [`docs/spec.md`](docs/spec.md) — Specification
- [`docs/development-manual.md`](docs/development-manual.md) — Developer guide
- [`docs/installation-manual.md`](docs/installation-manual.md) — Installation
- [`docs/configuration-manual.md`](docs/configuration-manual.md) — Configuration

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

See [SECURITY.md](.github/SECURITY.md).

## License

[MIT](LICENSE)
