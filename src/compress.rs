use std::io;

#[cfg(feature = "native-compress")]
use std::io::Write;

#[cfg(not(feature = "native-compress"))]
use std::io::BufReader;

#[cfg(not(feature = "native-compress"))]
use lzma_rs::xz_compress;

#[cfg(feature = "native-compress")]
use xz2::stream::{Check, Filters, LzmaOptions, MatchFinder, Mode, Stream};
#[cfg(feature = "native-compress")]
use xz2::write::XzEncoder;

#[cfg(feature = "native-compress")]
const LZMA_PRESET_EXTREME: u32 = 1 << 31;

pub fn compress_lzma(data: &[u8]) -> io::Result<Vec<u8>> {
    #[cfg(feature = "native-compress")]
    {
        compress_ultra(data)
    }

    #[cfg(not(feature = "native-compress"))]
    {
        let mut reader = BufReader::new(io::Cursor::new(data));
        let mut compressed = Vec::new();
        compress_xz_to(&mut reader, &mut compressed)?;
        Ok(compressed)
    }
}

/// Ultra compression: LZMA2 extreme preset 9 + 64 MiB dictionary +
/// BinaryTree4 + nice_len=273. No BCJ pre-filter — lzma-rs (used by
/// the stub for decompression) only supports the LZMA2 filter.
#[cfg(feature = "native-compress")]
fn compress_ultra(data: &[u8]) -> io::Result<Vec<u8>> {
    let map = io::Error::other;

    let mut opts = LzmaOptions::new_preset(9 | LZMA_PRESET_EXTREME).map_err(map)?;
    let dict = std::cmp::min(64 * 1024 * 1024, data.len().next_power_of_two() as u32);
    opts.dict_size(std::cmp::max(dict, 4096));
    opts.match_finder(MatchFinder::BinaryTree4);
    opts.mode(Mode::Normal);
    opts.nice_len(273);

    let mut filters = Filters::new();
    filters.lzma2(&opts);

    let stream = Stream::new_stream_encoder(&filters, Check::Crc64).map_err(map)?;
    let mut encoder = XzEncoder::new_stream(Vec::new(), stream);
    encoder.write_all(data)?;
    encoder.finish()
}

#[cfg(not(feature = "native-compress"))]
fn compress_xz_to<R: io::BufRead, W: io::Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    xz_compress(reader, writer).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lzma_rs::xz_decompress;
    use std::io::{BufReader, Cursor};

    #[cfg(not(feature = "native-compress"))]
    struct FailWriter;

    #[cfg(not(feature = "native-compress"))]
    impl io::Write for FailWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "forced failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_compress_nonempty() {
        let data = b"Hello, World!";
        let compressed = compress_lzma(data).unwrap();
        assert!(!compressed.is_empty());
    }

    #[test]
    fn test_compress_produces_valid_xz() {
        let data = b"test data for xz validation";
        let compressed = compress_lzma(data).unwrap();
        // XZ streams start with magic bytes FD 37 7A 58 5A 00
        assert!(compressed.len() >= 6);
        assert_eq!(&compressed[..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
    }

    #[test]
    fn test_compress_empty_input() {
        let compressed = compress_lzma(b"").unwrap();
        // Even empty input produces a valid XZ stream
        assert!(!compressed.is_empty());
        assert_eq!(&compressed[..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
    }

    #[test]
    fn test_compress_large_repetitive_data() {
        let data = vec![0x42u8; 100_000];
        let compressed = compress_lzma(&data).unwrap();
        assert!(!compressed.is_empty());
        assert_eq!(&compressed[..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_roundtrip() {
        let original = b"Round-trip test data for compression";
        let compressed = compress_lzma(original).unwrap();
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(&decompressed, original);
    }

    #[test]
    fn test_compress_roundtrip_binary_data() {
        let original: Vec<u8> = (0..=255).collect();
        let compressed = compress_lzma(&original).unwrap();
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_sec_compress_all_zeros() {
        let data = vec![0u8; 10_000];
        let compressed = compress_lzma(&data).unwrap();
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(decompressed, data);
    }

    #[cfg(not(feature = "native-compress"))]
    #[test]
    fn test_sec_compress_write_error() {
        let mut fw = FailWriter;
        assert!(fw.flush().is_ok());

        let data = b"test";
        let mut reader = BufReader::new(Cursor::new(data.as_slice()));
        let result = compress_xz_to(&mut reader, &mut fw);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_sec_compress_single_byte() {
        let data = [0xFFu8];
        let compressed = compress_lzma(&data).unwrap();
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_sec_compress_repeated_no_leak() {
        // Repeated compress cycles — must not accumulate resources or panic
        let data = vec![0x42u8; 10_000];
        for _ in 0..100 {
            let compressed = compress_lzma(&data).unwrap();
            assert!(!compressed.is_empty());
        }
    }

    #[test]
    fn test_sec_compress_null_payload() {
        // Payload entirely of null bytes
        let data = vec![0u8; 50_000];
        let compressed = compress_lzma(&data).unwrap();
        let mut decompressed = Vec::new();
        xz_decompress(
            &mut BufReader::new(Cursor::new(compressed)),
            &mut decompressed,
        )
        .unwrap();
        assert_eq!(decompressed, data);
    }
}
