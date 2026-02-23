use std::io::{BufReader, Cursor};

use xsfx::common::{Trailer, MAGIC, TRAILER_SIZE};
use xsfx::compress::compress_lzma;
use xsfx::decompress::decompress_payload;

// --- Positive path tests ---

#[test]
fn test_compress_decompress_roundtrip() {
    let payload = b"Integration test payload for SFX";
    let compressed = compress_lzma(payload).unwrap();
    let mut reader = BufReader::new(Cursor::new(&compressed));
    let decompressed = decompress_payload(&mut reader).unwrap();
    assert_eq!(decompressed, payload);
}

#[test]
fn test_sfx_format_assembly_and_parsing() {
    let stub = b"FAKE_STUB_BINARY";
    let payload = b"test payload data";
    let compressed = compress_lzma(payload).unwrap();
    let trailer = Trailer::new(compressed.len() as u64);

    // Assemble SFX: [stub][compressed][trailer]
    let mut sfx = Vec::new();
    sfx.extend_from_slice(stub);
    sfx.extend_from_slice(&compressed);
    sfx.extend_from_slice(&trailer.to_bytes());

    // Parse like the stub would
    let total_len = sfx.len() as u64;
    let trailer_offset = (total_len - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[trailer_offset..])).unwrap();
    assert_eq!(parsed.magic, MAGIC);
    assert_eq!(parsed.payload_len, compressed.len() as u64);

    // Extract and decompress
    let payload_start = trailer_offset - parsed.payload_len as usize;
    let compressed_slice = &sfx[payload_start..trailer_offset];
    let mut reader = BufReader::new(Cursor::new(compressed_slice));
    let result = decompress_payload(&mut reader).unwrap();
    assert_eq!(result, payload);
}

#[test]
fn test_various_payload_sizes() {
    for size in [0, 1, 10, 100, 1_000, 50_000] {
        let payload = vec![0xABu8; size];
        let compressed = compress_lzma(&payload).unwrap();
        let trailer = Trailer::new(compressed.len() as u64);

        let mut sfx = Vec::new();
        sfx.extend_from_slice(b"STUB");
        sfx.extend_from_slice(&compressed);
        sfx.extend_from_slice(&trailer.to_bytes());

        let total = sfx.len() as u64;
        let t_off = (total - TRAILER_SIZE) as usize;
        let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();
        assert_eq!(parsed.magic, MAGIC);

        let p_start = t_off - parsed.payload_len as usize;
        let mut reader = BufReader::new(Cursor::new(&sfx[p_start..t_off]));
        let result = decompress_payload(&mut reader).unwrap();
        assert_eq!(result, payload, "mismatch at size {}", size);
    }
}

#[test]
fn test_trailer_preserves_stub_offset() {
    let stub = vec![0xCCu8; 256];
    let payload = b"small";
    let compressed = compress_lzma(payload).unwrap();
    let trailer = Trailer::new(compressed.len() as u64);

    let mut sfx = Vec::new();
    sfx.extend_from_slice(&stub);
    sfx.extend_from_slice(&compressed);
    sfx.extend_from_slice(&trailer.to_bytes());

    let total = sfx.len() as u64;
    let t_off = (total - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();

    let stub_end = t_off - parsed.payload_len as usize;
    assert_eq!(stub_end, stub.len());
    assert_eq!(&sfx[..stub_end], &stub);
}

// --- Security / adversarial tests ---

#[test]
fn test_sec_corrupted_trailer_magic() {
    let mut trailer_bytes = Trailer::new(100).to_bytes();
    trailer_bytes[8] = 0x00; // corrupt first magic byte
    let t = Trailer::from_reader(Cursor::new(trailer_bytes)).unwrap();
    assert_ne!(t.magic, MAGIC);
}

#[test]
fn test_sec_corrupted_compressed_data() {
    let payload = b"good data";
    let mut compressed = compress_lzma(payload).unwrap();
    // Corrupt the middle of the compressed stream
    if compressed.len() > 20 {
        compressed[15] ^= 0xFF;
        compressed[16] ^= 0xFF;
    }
    let mut reader = BufReader::new(Cursor::new(compressed));
    let result = decompress_payload(&mut reader);
    assert!(result.is_err());
}

#[test]
fn test_sec_payload_length_exceeds_sfx() {
    let stub = b"STUB";
    let payload = b"data";
    let compressed = compress_lzma(payload).unwrap();
    // Claim a much larger payload than actually present
    let bad_trailer = Trailer::new(999_999);

    let mut sfx = Vec::new();
    sfx.extend_from_slice(stub);
    sfx.extend_from_slice(&compressed);
    sfx.extend_from_slice(&bad_trailer.to_bytes());

    let total = sfx.len() as u64;
    let t_off = (total - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();

    // Validate: payload_len > total file size = invalid
    assert!(parsed.payload_len > total);
}

#[test]
fn test_sec_zero_payload_length() {
    let trailer = Trailer::new(0);
    let bytes = trailer.to_bytes();
    let parsed = Trailer::from_reader(Cursor::new(bytes)).unwrap();
    assert_eq!(parsed.payload_len, 0);
    // Stub should reject payload_len == 0 as invalid
}

#[test]
fn test_sec_max_payload_length() {
    let trailer = Trailer::new(u64::MAX);
    let bytes = trailer.to_bytes();
    let parsed = Trailer::from_reader(Cursor::new(bytes)).unwrap();
    assert_eq!(parsed.payload_len, u64::MAX);
    // Stub should reject payload_len > total_len as invalid
}

#[test]
fn test_sec_sfx_with_no_compressed_data() {
    let stub = b"STUB";
    let trailer = Trailer::new(0);
    let mut sfx = Vec::new();
    sfx.extend_from_slice(stub);
    sfx.extend_from_slice(&trailer.to_bytes());

    let total = sfx.len() as u64;
    let t_off = (total - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();
    assert_eq!(parsed.payload_len, 0);
    assert_eq!(parsed.magic, MAGIC);
}

#[test]
fn test_sec_trailer_at_minimum_file_size() {
    // File is exactly 16 bytes (trailer only, no stub, no payload)
    let trailer = Trailer::new(0);
    let sfx = trailer.to_bytes();
    assert_eq!(sfx.len(), TRAILER_SIZE as usize);
    let parsed = Trailer::from_reader(Cursor::new(sfx)).unwrap();
    assert_eq!(parsed.magic, MAGIC);
}

#[test]
fn test_sec_binary_payload_roundtrip() {
    // Payload containing all possible byte values
    let payload: Vec<u8> = (0..=255).collect();
    let compressed = compress_lzma(&payload).unwrap();
    let mut reader = BufReader::new(Cursor::new(&compressed));
    let result = decompress_payload(&mut reader).unwrap();
    assert_eq!(result, payload);
}

#[test]
fn test_sec_payload_offset_underflow() {
    // payload_len larger than (total_len - TRAILER_SIZE) would underflow
    // the payload_start calculation: total_len - TRAILER_SIZE - payload_len
    let stub = b"STUB";
    let compressed = compress_lzma(b"x").unwrap();
    let sfx_payload_len = (stub.len() + compressed.len() + 1) as u64; // 1 byte too large
    let bad_trailer = Trailer::new(sfx_payload_len);

    let mut sfx = Vec::new();
    sfx.extend_from_slice(stub);
    sfx.extend_from_slice(&compressed);
    sfx.extend_from_slice(&bad_trailer.to_bytes());

    let total = sfx.len() as u64;
    let t_off = (total - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();

    // The payload_len exceeds the available space (stub + compressed)
    // Stub code must validate: payload_len <= total_len - TRAILER_SIZE
    let available = total - TRAILER_SIZE;
    assert!(parsed.payload_len > available);
}

#[test]
fn test_sec_null_byte_payload_full_roundtrip() {
    // All-null payload through full SFX assembly and extraction
    let stub = b"FAKESTUB";
    let payload = vec![0u8; 10_000];
    let compressed = compress_lzma(&payload).unwrap();
    let trailer = Trailer::new(compressed.len() as u64);

    let mut sfx = Vec::new();
    sfx.extend_from_slice(stub);
    sfx.extend_from_slice(&compressed);
    sfx.extend_from_slice(&trailer.to_bytes());

    let total = sfx.len() as u64;
    let t_off = (total - TRAILER_SIZE) as usize;
    let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();
    assert_eq!(parsed.magic, MAGIC);

    let p_start = t_off - parsed.payload_len as usize;
    let mut reader = BufReader::new(Cursor::new(&sfx[p_start..t_off]));
    let result = decompress_payload(&mut reader).unwrap();
    assert_eq!(result, payload);
}

#[test]
fn test_sec_sfx_assembly_repeated_no_leak() {
    // Repeated SFX assembly/parse cycles — no resource accumulation
    let stub = b"STUB";
    let payload = b"repeated test";
    let compressed = compress_lzma(payload).unwrap();

    for _ in 0..100 {
        let trailer = Trailer::new(compressed.len() as u64);
        let mut sfx = Vec::new();
        sfx.extend_from_slice(stub);
        sfx.extend_from_slice(&compressed);
        sfx.extend_from_slice(&trailer.to_bytes());

        let total = sfx.len() as u64;
        let t_off = (total - TRAILER_SIZE) as usize;
        let parsed = Trailer::from_reader(Cursor::new(&sfx[t_off..])).unwrap();
        assert_eq!(parsed.magic, MAGIC);

        let p_start = t_off - parsed.payload_len as usize;
        let mut reader = BufReader::new(Cursor::new(&sfx[p_start..t_off]));
        let result = decompress_payload(&mut reader).unwrap();
        assert_eq!(result, payload.as_slice());
    }
}

#[test]
fn test_sec_corrupted_every_trailer_byte() {
    // Corrupt each byte position in the trailer independently
    let original = Trailer::new(42);
    let original_bytes = original.to_bytes();
    for i in 0..16 {
        let mut corrupted = original_bytes;
        corrupted[i] ^= 0xFF;
        let parsed = Trailer::from_reader(Cursor::new(corrupted)).unwrap();
        // At least one field should differ from the original
        let differs = parsed.payload_len != 42 || parsed.magic != MAGIC;
        assert!(differs, "corruption at byte {} had no effect", i);
    }
}
