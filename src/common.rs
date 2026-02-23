use std::io::{self, Read};

pub const TRAILER_SIZE: u64 = 16;
// Just a random constant marker: "SFXLZMA!" in hex-like style
pub const MAGIC: u64 = 0x5346584C5A4D4121; // "SFXLZMA!"

pub struct Trailer {
    pub payload_len: u64,
    pub magic: u64,
}

impl Trailer {
    pub fn new(payload_len: u64) -> Self {
        Self {
            payload_len,
            magic: MAGIC,
        }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[..8].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[8..16].copy_from_slice(&self.magic.to_le_bytes());
        buf
    }

    pub fn from_reader<R: Read>(mut r: R) -> io::Result<Self> {
        let mut buf = [0u8; 16];
        r.read_exact(&mut buf)?;
        let mut len_bytes = [0u8; 8];
        let mut magic_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&buf[..8]);
        magic_bytes.copy_from_slice(&buf[8..16]);
        let payload_len = u64::from_le_bytes(len_bytes);
        let magic = u64::from_le_bytes(magic_bytes);
        Ok(Self { payload_len, magic })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_trailer_new_sets_magic() {
        let t = Trailer::new(42);
        assert_eq!(t.magic, MAGIC);
    }

    #[test]
    fn test_trailer_new_sets_payload_len() {
        let t = Trailer::new(12345);
        assert_eq!(t.payload_len, 12345);
    }

    #[test]
    fn test_trailer_to_bytes_length() {
        let t = Trailer::new(100);
        let bytes = t.to_bytes();
        assert_eq!(bytes.len(), TRAILER_SIZE as usize);
    }

    #[test]
    fn test_trailer_to_bytes_le_encoding() {
        let t = Trailer::new(1);
        let bytes = t.to_bytes();
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1..8], [0; 7]);
        assert_eq!(&bytes[8..16], &MAGIC.to_le_bytes());
    }

    #[test]
    fn test_trailer_roundtrip() {
        let original = Trailer::new(99999);
        let bytes = original.to_bytes();
        let restored = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(restored.payload_len, 99999);
        assert_eq!(restored.magic, MAGIC);
    }

    #[test]
    fn test_trailer_zero_payload() {
        let t = Trailer::new(0);
        let bytes = t.to_bytes();
        let restored = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(restored.payload_len, 0);
        assert_eq!(restored.magic, MAGIC);
    }

    #[test]
    fn test_trailer_max_payload() {
        let t = Trailer::new(u64::MAX);
        let bytes = t.to_bytes();
        let restored = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(restored.payload_len, u64::MAX);
        assert_eq!(restored.magic, MAGIC);
    }

    #[test]
    fn test_trailer_size_constant() {
        assert_eq!(TRAILER_SIZE, 16);
    }

    #[test]
    fn test_magic_constant() {
        assert_eq!(MAGIC, 0x5346584C5A4D4121);
        // "SFXLZMA!" in ASCII
        let bytes = MAGIC.to_be_bytes();
        assert_eq!(&bytes, b"SFXLZMA!");
    }

    #[test]
    fn test_sec_trailer_from_reader_truncated() {
        let result = Trailer::from_reader(Cursor::new([0u8; 8]));
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_trailer_from_reader_empty() {
        let result = Trailer::from_reader(Cursor::new([]));
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_trailer_invalid_magic_parsed() {
        let mut bytes = Trailer::new(100).to_bytes();
        bytes[8] = 0xFF;
        let t = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_ne!(t.magic, MAGIC);
    }

    #[test]
    fn test_sec_trailer_all_zeros() {
        let bytes = [0u8; 16];
        let t = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(t.payload_len, 0);
        assert_ne!(t.magic, MAGIC);
    }

    #[test]
    fn test_sec_trailer_all_ones() {
        let bytes = [0xFFu8; 16];
        let t = Trailer::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(t.payload_len, u64::MAX);
        assert_ne!(t.magic, MAGIC);
    }

    #[test]
    fn test_sec_trailer_repeated_no_leak() {
        // Repeated trailer create/parse cycles — no resource accumulation
        for i in 0..1000u64 {
            let t = Trailer::new(i);
            let bytes = t.to_bytes();
            let parsed = Trailer::from_reader(Cursor::new(bytes)).unwrap();
            assert_eq!(parsed.payload_len, i);
            assert_eq!(parsed.magic, MAGIC);
        }
    }
}
