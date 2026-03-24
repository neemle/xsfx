# xsfx — Installation Manual

## 1. Download Pre-built Binaries

Pre-built binaries are available from [GitHub Releases](https://github.com/ratushnyi-labs/xsfx/releases).

Available platforms:

| Platform | File |
|----------|------|
| Linux x64 (static) | `xsfx-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 (static) | `xsfx-aarch64-unknown-linux-musl.tar.gz` |
| macOS x64 | `xsfx-x86_64-apple-darwin.tar.gz` |
| macOS ARM64 (Apple Silicon) | `xsfx-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `xsfx-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `xsfx-aarch64-pc-windows-msvc.zip` |

## 2. Platform-Specific Installation

### Linux / macOS

```bash
# Download and extract (example: Linux x64)
curl -sSfL https://github.com/ratushnyi-labs/xsfx/releases/latest/download/xsfx-x86_64-unknown-linux-musl.tar.gz \
    | tar xzf - -C /usr/local/bin

# Verify
xsfx --help
```

### Windows

1. Download the `.zip` for your architecture from the releases page
2. Extract `xsfx.exe` to a directory in your `PATH`
3. Verify: `xsfx --help`

## 3. Build from Source

### Prerequisites

- Rust 1.76+ (`rustup install stable`)
- C compiler (gcc, clang, or MSVC)

### Build

```bash
git clone https://github.com/ratushnyi-labs/xsfx.git
cd xsfx
cargo build --release --bin xsfx --features native-compress
```

The binary will be at `target/release/xsfx` (or `target\release\xsfx.exe` on Windows).

### Build without native liblzma

```bash
cargo build --release --bin xsfx --no-default-features
```

This uses the pure-Rust lzma-rs for compression (lower compression ratio, no C compiler needed).

## 4. Usage

### Pack a binary

```bash
xsfx <input_payload> <output_sfx>
```

Example:

```bash
xsfx myapp myapp-sfx
chmod +x myapp-sfx
./myapp-sfx
```

### Pack for a different target

```bash
xsfx myapp myapp-sfx.exe --target x86_64-pc-windows-msvc
```

### List available targets

```bash
xsfx --list-targets
```

### Pipe support

Use `-` for stdin or stdout:

```bash
# Read payload from stdin
cat myapp | xsfx - myapp-sfx

# Write SFX to stdout
xsfx myapp - > myapp-sfx

# Both — full pipe
cat myapp | xsfx - - > myapp-sfx

# Pipe over SSH
xsfx myapp - --target x86_64-unknown-linux-musl | ssh server 'cat > /usr/local/bin/myapp-sfx && chmod +x /usr/local/bin/myapp-sfx'
```

### Run the packed SFX

The output SFX binary runs like any normal executable. All CLI arguments are forwarded to the payload:

```bash
./myapp-sfx --verbose --config /etc/myapp.conf
```

## 5. Verification

After packing, verify the SFX works correctly:

```bash
# Pack
xsfx myapp myapp-sfx

# Run original
./myapp --version

# Run SFX — should produce identical output
./myapp-sfx --version
```

## 6. Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| `"Invalid SFX magic marker"` | Corrupted SFX binary | Re-pack from original payload |
| `"File too small to contain trailer"` | Truncated SFX file | Re-download or re-pack |
| `Permission denied` | Missing execute permission | `chmod +x <sfx>` |
| `memfd_create: Operation not permitted` | Kernel restricts memfd in container | Ensure `SYS_PTRACE` cap or use a kernel >= 3.17 |
| Windows: `"Failed to load DLL"` | Missing runtime DLL dependency | Install required Visual C++ redistributable |
| macOS: `"Failed to create object file image"` | macOS code signing issue | Sign the SFX binary or allow unsigned execution |
