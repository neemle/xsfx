Rust SFX Packer
================

Self-extracting executable builder written in Rust. It embeds a small per-platform stub that unpacks and runs a compressed payload at runtime. By default builds are pure Rust; enabling the optional `native-compress` feature uses `liblzma` via `xz2` for smaller payloads during packing.

Features
- Pure-Rust stub (lzma-rs) for macOS, Linux, and Windows.
- Optional native compression for better ratios when building the packer.
- Multi-stub packer: a single xsfx binary can embed stubs for many OS/arch combinations and produce SFX for any of them.
- Single-command packing: `xsfx <payload> <output_sfx> [--target <triple>]`.

Prerequisites
- Docker (required for the unified build; all toolchains run in a Linux container).
- Payload binary built for the same target as the chosen stub (e.g., Linux payload for Linux stub).

Packer CLI
```bash
xsfx <payload_path> <output_sfx> [--target <triple>]
```
Notes:
- If `--target` is omitted, xsfx defaults to the build triple captured at compile time.
- This repository’s build script embeds a catalog of stubs; you can control which are included with `XSFX_TARGETS` during build.
  - By default we aim for a common cross-platform set: Linux (GNU, MUSL) x86_64/aarch64, macOS x86_64/aarch64, Windows (GNU+MSVC) x86_64/aarch64.
  - You can also provide prebuilt stubs using `XSFX_PREBUILT_STUBS_DIR` (see below) to avoid cross-compiling them locally.

Compression
- Default: pure-Rust lzma-rs encoder (slower/weaker compression, smallest vector of dependencies).
- Better ratios: build packer with `--features native-compress` to use liblzma via `xz2` during packing; the stub remains pure Rust.

Unified build image (build once, reuse many times)
-------------------------------------------------
We provide a Dockerfile that bakes all required toolchains (GNU/MUSL, aarch64, MinGW-w64, osxcross + macOS SDK) and Rust targets. Build it once:

```bash
# Optional: override SDK via build-arg
docker build --platform linux/amd64 -t xsfx-build \
  --build-arg MAC_SDK_URL=https://github.com/alexey-lysiuk/macos-sdk/releases/download/15.5/MacOSX15.5.tar.xz \
  .
```

Unified build (single command, outputs go to ./dist)
---------------------------------------------------
Run this one command and the script will use the prebuilt image (building it if missing) to produce ALL supported packer variants into `./dist`. The Docker image now contains the build logic as an entrypoint script, so the host-side build.sh is minimal and easy to read/maintain:

```bash
./build.sh
```

What it builds inside the container:
- Linux: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
- macOS: x86_64-apple-darwin, aarch64-apple-darwin (via osxcross inside the container)
- Windows: x86_64-pc-windows-gnu

Notes/limits:
- Windows MSVC targets (x86_64-pc-windows-msvc, aarch64-pc-windows-msvc) are not built in the Linux container due to MSVC toolchain restrictions. You can provide those stubs prebuilt if needed (see below) and they will still be embedded into the packer catalog.

Environment variables:
- `XSFX_TARGETS`: comma-separated list of target triples to embed as stubs inside the xsfx packer. You may also set it to `all` to include the default full set. If omitted or set to `all`, all common targets are included by default (Linux GNU+MUSL x86_64/aarch64, macOS x86_64/aarch64, Windows GNU x86_64, Windows MSVC x86_64/aarch64).
- `XSFX_TARGET`: alias for `XSFX_TARGETS`. You may set it to a comma-separated list or simply `all` to include the default full set. If both are set, `XSFX_TARGETS` takes precedence.
- `XSFX_PREBUILT_STUBS_DIR`: optional directory with prebuilt stubs to embed instead of cross-compiling them. Accepted layouts per target triple `<triple>`:
  - `<dir>/<triple>/stub` or `<dir>/<triple>/stub.exe`
  - `<dir>/<triple>-stub` or `<dir>/<triple>-stub.exe`
 - `XSFX_MAC_SDK_URL`: override the default macOS SDK download URL used by osxcross when building the Docker image or when running `./build.sh` the first time (defaults to macOS 15.5 SDK: https://github.com/alexey-lysiuk/macos-sdk/releases/download/15.5/MacOSX15.5.tar.xz). When building the image manually, pass it as `--build-arg MAC_SDK_URL=...`.

Notes for Apple Silicon (M1/M2/M3):
- The build uses linux/amd64 images for maximum availability. Docker Desktop uses emulation automatically; no host Rust toolchains are installed.

Outputs:
- The script writes packer binaries into `./dist` named `xsfx-<triple>` or `xsfx-<triple>.exe`.

Build macOS binaries happens automatically inside the Docker container using osxcross.
The script downloads the SDK and builds osxcross unless `/opt/osxcross/target/bin` already exists in the container layer.

Tip: Supplying additional prebuilt stubs
- If you have prebuilt Windows MSVC or other stubs from native hosts, you can embed them into the packer by placing them in a directory and setting `XSFX_PREBUILT_STUBS_DIR` before running a standard Cargo build (outside the container), or adapt the container script to mount them at `/project/.build/stubs`.

Troubleshooting
- “Permission denied” when running SFX on Unix: `chmod +x dist/your-sfx`.
- Payload fails to launch: ensure the payload matches the target you chose with `--target` (architecture + OS).
- If a requested target isn’t available in the built packer, rerun the build with `XSFX_TARGETS=...` (and/or provide prebuilt stubs with `XSFX_PREBUILT_STUBS_DIR`).
- Building inside Docker without prebuilt stubs: some stub targets may be skipped if their toolchains/linkers are unavailable in the container. The build will continue and embed all stubs that succeed. To get a fully populated catalog (including macOS and MSVC), provide those stubs via `XSFX_PREBUILT_STUBS_DIR` produced on native hosts.
 - Error "could not find specification for target 'aarch64-pc-windows-gnu'": this target does not exist in Rust. Windows ARM64 is MSVC-only; use `aarch64-pc-windows-msvc`. The unified build script excludes the invalid GNU ARM64 Windows target.
