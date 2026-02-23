use std::io;

use lzma_rs::xz_decompress;

pub fn decompress_payload<R: io::BufRead>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut payload = Vec::new();
    xz_decompress(reader, &mut payload).map_err(|_| io::Error::other("decompression failed"))?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::compress_lzma;
    use std::io::{BufReader, Cursor};

    #[test]
    fn test_decompress_valid() {
        let original = b"Hello, decompression!";
        let compressed = compress_lzma(original).unwrap();
        let mut reader = BufReader::new(Cursor::new(compressed));
        let result = decompress_payload(&mut reader).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decompress_roundtrip_various_sizes() {
        for size in [0, 1, 100, 1000, 10_000] {
            let original = vec![0xABu8; size];
            let compressed = compress_lzma(&original).unwrap();
            let mut reader = BufReader::new(Cursor::new(compressed));
            let result = decompress_payload(&mut reader).unwrap();
            assert_eq!(result, original, "failed for size {}", size);
        }
    }

    #[test]
    fn test_sec_decompress_invalid_data() {
        let bad_data = vec![0xFF; 100];
        let mut reader = BufReader::new(Cursor::new(bad_data));
        let result = decompress_payload(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_decompress_empty_input() {
        let mut reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
        let result = decompress_payload(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_decompress_truncated_xz() {
        let original = b"data to truncate";
        let compressed = compress_lzma(original).unwrap();
        let truncated = &compressed[..compressed.len() / 2];
        let mut reader = BufReader::new(Cursor::new(truncated));
        let result = decompress_payload(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_decompress_partial_xz_header() {
        // Only the XZ magic bytes, nothing else
        let partial = vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
        let mut reader = BufReader::new(Cursor::new(partial));
        let result = decompress_payload(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_decompress_random_bytes() {
        let random: Vec<u8> = (0..256).map(|i| (i * 37 + 13) as u8).collect();
        let mut reader = BufReader::new(Cursor::new(random));
        let result = decompress_payload(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_decompress_high_ratio() {
        // Compress 1 MB of zeros — high compression ratio stress test
        let original = vec![0u8; 1_000_000];
        let compressed = compress_lzma(&original).unwrap();
        assert!(compressed.len() < 1000, "sanity: high ratio expected");
        let mut reader = BufReader::new(Cursor::new(&compressed));
        let result = decompress_payload(&mut reader).unwrap();
        assert_eq!(result.len(), 1_000_000);
        assert_eq!(result, original);
    }

    #[test]
    fn test_sec_decompress_null_bytes_payload() {
        // Payload entirely of null bytes through compress/decompress roundtrip
        let original = vec![0u8; 10_000];
        let compressed = compress_lzma(&original).unwrap();
        let mut reader = BufReader::new(Cursor::new(&compressed));
        let result = decompress_payload(&mut reader).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_sec_decompress_repeated_no_leak() {
        // Repeated decompress cycles — must not accumulate resources or panic
        let original = vec![0xABu8; 5_000];
        let compressed = compress_lzma(&original).unwrap();
        for _ in 0..100 {
            let mut reader = BufReader::new(Cursor::new(&compressed));
            let result = decompress_payload(&mut reader).unwrap();
            assert_eq!(result.len(), 5_000);
        }
    }
}
