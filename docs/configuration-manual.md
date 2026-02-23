# Configuration Manual

## Build-time Configuration

xsfx has no runtime configuration. All settings are applied at build time.

### Environment Variables

| Variable         | Required | Description                                    |
|------------------|----------|------------------------------------------------|
| `XSFX_TARGETS` | No | Comma-separated list of targets to build stubs for. `all` for all targets. Defaults to host target. |
| `XSFX_PREBUILT_STUBS_DIR` | No | Directory of pre-built stubs (skips building). Used by Docker cross-build. |
| `XSFX_SKIP_STUB_BUILD` | No | Set to `1` to skip stub building and generate empty catalog. Used by test stage. |
| `XSFX_OUT_TARGET` | No | Override the default target triple when running the packer. |

### Cargo Features

| Feature            | Default | Description                                   |
|--------------------|---------|-----------------------------------------------|
| `native-compress`  | **On**  | Use liblzma via xz2 for ultra compression (LZMA2 extreme preset 9, 64 MiB dictionary). liblzma is statically linked from vendored source — no system `liblzma-dev` needed. The stub always uses pure-Rust lzma-rs regardless. |

### Release Profile

The release profile is optimized for minimal binary size:

| Setting        | Value   | Effect                                 |
|----------------|---------|----------------------------------------|
| `opt-level`    | `"z"`   | Optimize for size                      |
| `lto`          | `true`  | Link-time optimization                 |
| `codegen-units`| `1`     | Better cross-crate optimization        |
| `panic`        | `abort` | Remove unwinding code                  |
| `strip`        | `true`  | Remove debug symbols (Rust 1.76+)      |

### Stub Build Pipeline

The stub uses a multi-stage minification pipeline to stay under 100 KB:

| Stage | Technique | Effect |
|-------|-----------|--------|
| 1. Nightly build-std | `-Z build-std=std,panic_abort` | Rebuilds std optimized for size, dead code elimination |
| 2. Immediate abort | `-Cpanic=immediate-abort` | Eliminates all panic formatting machinery |
| 3. Format removal | No `eprintln!`/`format!` in stub code | Avoids pulling in `core::fmt` infrastructure |
| 4. UPX compression | `upx --best --lzma` post-build | LZMA-compressed executable with tiny decompressor header |
| 4a. xstrip (musl only) | `xstrip -i` post-build | ELF dead-code removal for musl stubs where UPX is skipped |

Result: ~170 KB pre-UPX -> ~77 KB post-UPX (Windows x64).

UPX is skipped for `*-linux-musl` stubs (see SDD-004). Instead, xstrip
is applied for ELF-level dead code removal. For musl targets, a two-stage
SFX format can also be used for further size reduction (see below).

### Two-Stage SFX Pipeline (musl)

For `*-linux-musl` targets where UPX cannot be used, a two-stage format
wraps the entire traditional SFX inside a tiny nostd loader:

| Stage | Component | Size | Description |
|-------|-----------|------|-------------|
| 0 | stage0 loader | ~10 KB | `#![no_std]`, raw syscalls, custom RFC 1951 inflate |
| 1 | stub + xz(payload) + trailer | ~96 KB + payload | Standard SFX, deflate-compressed inside stage0 |

The stage0 loader reads its own trailer, inflates the stage1 SFX from
a deflate stream into a memfd, and execveat's it. The stub opens itself
via `/proc/self/exe` (which the kernel resolves to the memfd).

Result: ~122 KB traditional -> ~73 KB two-stage (40% reduction, hello-world payload).

## Platform-specific Behavior

| Platform | Execution Strategy | Mechanism | Static Linking |
|----------|-------------------|-----------|----------------|
| Linux    | memfd_create (in-memory) | Anonymous memory fd + execveat(AT_EMPTY_PATH) | musl libc |
| Windows  | In-process PE loading | VirtualAlloc + import resolution + relocation fixing | crt-static |
| macOS    | NSObjectFileImage (in-memory) | Patch MH_EXECUTE to MH_BUNDLE, load via dyld API | System frameworks |

No temp files are used on any platform. On Linux, the stub opens its own
executable via `/proc/self/exe` directly (not `current_exe()`) so that it
works when running from a memfd (two-stage SFX).

## Compression Settings

### Packer (with `native-compress`)

| Setting | Value | Description |
|---------|-------|-------------|
| Preset | 9 + EXTREME | Maximum compression effort |
| Dictionary | 64 MiB (capped to input size) | Larger dictionary = better compression for large files |
| Match finder | BinaryTree4 | Best compression ratio |
| Nice length | 273 | Maximum match length |
| Pre-filter | None | BCJ removed — lzma-rs only supports LZMA2 filter |
| Check | CRC-64 | Integrity verification |

### Packer (without `native-compress`)

Uses pure-Rust lzma-rs with default XZ settings. Produces larger output but has
no native dependency.

### Stub (decompression)

Always uses pure-Rust lzma-rs `xz_decompress`. Compatible with both standard and
ultra-compressed XZ streams.

## Cross-Build Architecture

All targets are built from a single Linux Docker container (`Dockerfile`, stage `cross`):

| Component | Toolchain |
|-----------|-----------|
| Linux gnu (x64/arm64) | gcc / aarch64-linux-gnu-gcc |
| Linux musl (x64/arm64) | musl-gcc / aarch64-linux-musl-gcc |
| macOS (x64/arm64) | osxcross (o64-clang / oa64-clang) |
| Windows gnu (x64) | x86_64-w64-mingw32-gcc |
| Windows MSVC (x64/arm64) | clang-cl + lld-link + xwin SDK |

Build pipeline: `./build.sh` builds the Docker image then runs the entrypoint
which cross-compiles all stubs (nightly + build-std + UPX/xstrip) and all packers.

## CI Configuration

See `.github/workflows/ci.yml`. The CI pipeline runs:

1. **Test (Docker):** `docker compose run --build test` — fmt, clippy, tests
   with 100% line/function coverage, and `cargo audit` inside a lightweight container.
2. **Build (Docker):** Single ubuntu runner builds the cross-compilation image, then
   cross-compiles all 9 targets inside it. Stubs use nightly build-std + UPX.

Fail-fast order: test must pass before build starts.
