# SDD-006 — Drop x86 BCJ filter from native compression

**Impacted UCs:** UC-001, UC-002
**Impacted BR/WF:** BR-004, BR-005, WF-001, WF-002

## Problem

The packer's `native-compress` path applies an x86 BCJ pre-filter
(`filters.x86()`, XZ filter ID `0x04`) before LZMA2 compression. The stub
decompresses with pure-Rust `lzma-rs`, which only supports the LZMA2 filter
(ID `0x21`). Any other filter ID causes `Unknown filter id 4` and immediate
failure. This means every SFX built with `native-compress` (the default for
release builds) is broken — the stub cannot decompress the payload.

## Scope

- Remove the `filters.x86()` call from `compress_ultra()` in `compress.rs`
- Update BR-004 in `docs/spec.md` to remove BCJ filter reference
- Ensure roundtrip tests cover the native-compress path

## Non-goals

- Implementing BCJ filter support in the stub (complex, lzma-rs upstream
  has no plans to add it)
- Switching the stub to native liblzma (violates BR-005: zero native deps)
- Applying BCJ outside the XZ container (changes SFX format)

## Acceptance Criteria

- AC-1: Packer with `native-compress` produces XZ streams using LZMA2 only
  (no BCJ filter in the block header)
- AC-2: Stub's `lzma-rs` decompressor successfully decompresses payloads
  produced by the native packer
- AC-3: Compression still uses extreme preset 9, 64 MiB dictionary,
  BinaryTree4, nice_len=273 (only the BCJ pre-filter is removed)

## Security Acceptance Criteria (mandatory)

- SEC-1: No change to security posture — payload data is still compressed
  with LZMA2 at maximum settings

## Failure Modes / Error Mapping

| Condition                      | Behavior                              |
|--------------------------------|---------------------------------------|
| Decompression fails (lzma-rs)  | Print "SFX stub error", exit 1        |

## Test Matrix (mandatory)

| AC    | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod API | Prod Fullstack |
|-------|------|-------------|----------|---------|----|---------------|----------------|
| AC-1  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| AC-2  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |
| AC-3  | N/A  | N/A         | N/A      | N/A     | N/A| N/A           | N/A            |

Notes:
- The roundtrip tests in `compress.rs` verify AC-1 + AC-2 when run with
  `--features native-compress`. The CI test stage runs without the feature
  (using pure-Rust compression) but the cross-build pipeline verifies
  end-to-end with native-compress.
