# xsfx — Configuration Manual

## 1. Environment Variables

### Build-time (build.rs)

| Variable | Default | Description |
|----------|---------|-------------|
| `XSFX_TARGETS` | Host triple (or "all" in cross build) | Comma-separated list of target triples to build stubs for. Use `"all"` for all 9 targets. |
| `XSFX_TARGET` | — | Alias for `XSFX_TARGETS` (single target convenience) |
| `XSFX_PREBUILT_STUBS_DIR` | — | Path to directory containing pre-built stub binaries. Skips stub compilation when set. |
| `XSFX_SKIP_STUB_BUILD` | — | Set to `"1"` to skip stub building entirely (for tests, clippy, library-only builds). |

### Runtime (packer)

| Variable | Default | Description |
|----------|---------|-------------|
| `XSFX_OUT_TARGET` | Build-time default | Override the default target triple for `--target` when not specified on CLI. |

### Build orchestration (build.sh)

| Variable | Default | Description |
|----------|---------|-------------|
| `PACKER_TARGETS` | All 9 targets | Comma-separated list of packer targets to build (subset of 9). |

### Docker build (Dockerfile)

| Variable | Default | Description |
|----------|---------|-------------|
| `EXTRA_CA_CERTS` | — | PEM-encoded CA certificate chain to append to the container's trust store. |
| `MAC_SDK_URL` | MacOSX15.2 SDK | URL to download the macOS SDK for osxcross. |

## 2. Cargo Features

| Feature | Default | Description |
|---------|---------|-------------|
| `native-compress` | On | Use native liblzma (statically linked from vendored source) for LZMA2 ultra compression. Provides ~55% compression ratio. Requires a C compiler at build time. |

To disable (pure-Rust compression only):

```bash
cargo build --no-default-features
```

## 3. Release Profile

Configured in `Cargo.toml`:

| Setting | Value | Purpose |
|---------|-------|---------|
| `opt-level` | `"z"` | Optimize for binary size |
| `lto` | `true` | Link-time optimization |
| `codegen-units` | `1` | Better cross-crate optimization |
| `panic` | `"abort"` | Remove unwinding code |
| `strip` | `true` | Strip debug symbols |

## 4. Compression Settings

### Packer (with `native-compress`)

| Setting | Value |
|---------|-------|
| Preset | 9 + EXTREME (`9 \| 1<<31`) |
| Dictionary | 64 MiB (capped to input size, min 4 KiB) |
| Match finder | BinaryTree4 |
| Mode | Normal |
| Nice length | 273 |
| Filter | LZMA2 only (no BCJ pre-filter) |
| Check | CRC-64 |

### Packer (without `native-compress`)

Standard lzma-rs XZ compression with default settings.

### Stub (decompression)

Always pure-Rust lzma-rs. Compatible with both standard and ultra-compressed XZ streams.

## 5. Stub Build Pipeline

The stub optimization pipeline (run in the cross-build Docker container):

1. **Nightly Rust** with `-Z build-std=std,panic_abort` — rebuilds std with abort-only panic
2. **`-Cpanic=immediate-abort`** — eliminates all panic formatting code
3. **`--no-default-features`** — stub uses pure-Rust lzma-rs only
4. **Post-processing per target:**
   - Non-musl targets: `upx --best --lzma` compression
   - Musl targets: `xstrip` ELF dead-code removal (UPX incompatible due to AT_BASE auxiliary vector issue)
5. **Size target:** < 100 KB per stub

## 6. Static Linking Configuration

| Target | Linker | Static Linking Method |
|--------|--------|----------------------|
| `x86_64-unknown-linux-gnu` | `cc` | Default (glibc dynamic) |
| `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu-gcc` | Default |
| `x86_64-unknown-linux-musl` | `musl-gcc` | `-C target-feature=+crt-static` |
| `aarch64-unknown-linux-musl` | `aarch64-linux-musl-gcc` (zig) | `-C target-feature=+crt-static` |
| `x86_64-pc-windows-gnu` | `x86_64-w64-mingw32-gcc` | `-C target-feature=+crt-static` |
| `x86_64-pc-windows-msvc` | `lld-link` | `-C target-feature=+crt-static` |
| `aarch64-pc-windows-msvc` | `lld-link` | `-C target-feature=+crt-static` |
| `x86_64-apple-darwin` | `o64-clang` | System frameworks |
| `aarch64-apple-darwin` | `oa64-clang` | System frameworks |

## 7. Cross-Build Toolchain

The Docker cross-build stage (`rust:1.93.1-bookworm`) includes:

| Tool | Version | Purpose |
|------|---------|---------|
| Zig | 0.15.2 | aarch64-linux-musl cross-compilation |
| osxcross | Latest + macOS 15.2 SDK | macOS cross-compilation |
| xwin | 0.8.0 | MSVC cross-compilation (SDK splat) |
| UPX | 5.0.1 | Post-build stub compression |
| xstrip | 0.1.0 | ELF dead-code removal for musl stubs |
| mingw-w64 | System package | Windows GNU target |
| GCC aarch64 | System package | Linux ARM64 cross-compilation |
| musl-tools | System package | Linux musl x64 |

## 8. CI Pipeline

### Test Job (ci.yml)

Runs `docker compose run --build test` on every push to main/master and all PRs.

Fail-fast order:
1. Format check (`cargo fmt`)
2. Lint (`cargo clippy -D warnings`)
3. Coverage (100% lines + functions + regions, excluding `bin/`)
4. Security audit (`cargo audit`)

### Build Job (ci.yml)

Depends on test job passing. Cross-builds all 9 targets with 120-minute timeout.

Docker image caching: SHA256 of `Dockerfile` + `scripts/xsfx-entrypoint.sh` used as cache key, stored in `ghcr.io`.

Artifacts uploaded with 7-day retention.

### Release (release.yml)

Triggered on `v*` tags. Same cross-build process, then packages:
- Unix targets → `.tar.gz`
- Windows targets → `.zip`

Creates GitHub Release with auto-generated notes.
