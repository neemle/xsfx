# xsfx

Self-extracting executable packer written in Rust. Compresses a payload binary with LZMA/XZ and bundles it with a per-platform stub that decompresses and executes it in memory at runtime. No temporary files are written on any platform.

Does not modify PE headers, so packed .NET assemblies and other header-sensitive executables remain valid.

## Supported Targets

Each packer binary embeds stubs for all 9 targets. The target is selected at pack time via `--target`.

| Target | Arch | Execution Method |
|---|---|---|
| `x86_64-unknown-linux-gnu` | x64 | `memfd_create` |
| `aarch64-unknown-linux-gnu` | ARM64 | `memfd_create` |
| `x86_64-unknown-linux-musl` | x64 | `memfd_create` |
| `aarch64-unknown-linux-musl` | ARM64 | `memfd_create` |
| `x86_64-apple-darwin` | x64 | `NSCreateObjectFileImageFromMemory` |
| `aarch64-apple-darwin` | ARM64 | `NSCreateObjectFileImageFromMemory` |
| `x86_64-pc-windows-gnu` | x64 | In-process PE loader |
| `x86_64-pc-windows-msvc` | x64 | In-process PE loader |
| `aarch64-pc-windows-msvc` | ARM64 | In-process PE loader |

## Usage

```
xsfx <payload> <output_sfx> [--target <triple>]
```

- `payload` -- input binary to pack
- `output_sfx` -- output path for the self-extracting executable
- `--target` -- target triple (defaults to the packer's host platform)

Running the packer without arguments lists available targets.

All CLI arguments passed to the SFX at runtime are forwarded to the payload.

## Build

### Cross-build (Docker)

```bash
./build.sh
```

Builds packers for all 9 targets into `./dist/`. Requires Docker. The toolchain image is built on first run and cached for subsequent builds.

To build a subset of targets:

```bash
PACKER_TARGETS="x86_64-unknown-linux-gnu x86_64-pc-windows-msvc" ./build.sh
```

### Native build (single target)

```bash
cargo build --release --bin xsfx --features native-compress
```

Builds a packer for the host platform. The host stub is compiled by `build.rs`.

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
| Default | _(none)_ | Pure-Rust `lzma-rs` encoder |
| Native | `--features native-compress` | `liblzma` via `xz2` -- LZMA2 preset 9 extreme, x86 BCJ filter, 64 MiB dictionary |

The stub uses the pure-Rust `lzma-rs` decoder in both modes.

## Prerequisites

- **Docker** -- cross-building via `./build.sh`
- **Rust stable** -- native single-target builds
- **Rust nightly** -- used inside the Docker image for stub size optimization (`-Z build-std`, `panic=immediate-abort`)

## Troubleshooting

| Problem | Resolution |
|---|---|
| Permission denied on Unix | `chmod +x ./your-sfx` |
| Payload fails to execute | Verify the payload was built for the same OS and architecture as the `--target` |
| Stub exceeds 100 KB | Confirm nightly toolchain and UPX are present in the build image |

## License

MIT
