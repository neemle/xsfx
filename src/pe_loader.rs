use std::io;

// PE format constants
const DOS_MAGIC: u16 = 0x5A4D; // "MZ"
const PE_SIGNATURE: u32 = 0x0000_4550; // "PE\0\0"
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const OPTIONAL_MAGIC_PE32_PLUS: u16 = 0x020B;
#[cfg(target_os = "windows")]
const IMAGE_REL_BASED_DIR64: u16 = 10;
#[cfg(target_os = "windows")]
const IMAGE_REL_BASED_ABSOLUTE: u16 = 0;

#[cfg(any(target_os = "windows", test))]
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
#[cfg(any(target_os = "windows", test))]
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
#[cfg(any(target_os = "windows", test))]
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

// Windows memory allocation constants
#[cfg(target_os = "windows")]
const MEM_COMMIT: u32 = 0x1000;
#[cfg(target_os = "windows")]
const MEM_RESERVE: u32 = 0x2000;
#[cfg(target_os = "windows")]
const MEM_RELEASE: u32 = 0x8000;
#[cfg(target_os = "windows")]
const PAGE_READWRITE: u32 = 0x04;
#[cfg(target_os = "windows")]
const PAGE_READONLY: u32 = 0x02;
#[cfg(target_os = "windows")]
const PAGE_EXECUTE_READ: u32 = 0x20;
#[cfg(target_os = "windows")]
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
#[cfg(target_os = "windows")]
const PAGE_EXECUTE: u32 = 0x10;
#[cfg(target_os = "windows")]
const PAGE_NOACCESS: u32 = 0x01;

#[cfg(target_os = "windows")]
extern "system" {
    fn VirtualAlloc(addr: usize, size: usize, alloc_type: u32, protect: u32) -> *mut u8;
    fn VirtualProtect(addr: *mut u8, size: usize, new_protect: u32, old: *mut u32) -> i32;
    fn VirtualFree(addr: *mut u8, size: usize, free_type: u32) -> i32;
    fn LoadLibraryA(name: *const u8) -> usize;
    fn GetProcAddress(module: usize, name: *const u8) -> usize;
    fn FlushInstructionCache(process: usize, addr: *const u8, size: usize) -> i32;
    fn GetCurrentProcess() -> usize;
}

/// Parsed PE header information needed for loading.
#[derive(Debug)]
pub struct PeHeaders {
    pub image_base: u64,
    pub size_of_image: u32,
    pub entry_point_rva: u32,
    pub section_alignment: u32,
    pub sections: Vec<SectionInfo>,
    pub import_dir_rva: u32,
    pub import_dir_size: u32,
    pub reloc_dir_rva: u32,
    pub reloc_dir_size: u32,
}

/// Parsed section header.
#[derive(Debug)]
pub struct SectionInfo {
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_data_offset: u32,
    pub raw_data_size: u32,
    pub characteristics: u32,
}

fn read_u16(data: &[u8], offset: usize) -> io::Result<u16> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "offset overflow"))?;
    if end > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PE read out of bounds",
        ));
    }
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

fn read_u32(data: &[u8], offset: usize) -> io::Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "offset overflow"))?;
    if end > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PE read out of bounds",
        ));
    }
    Ok(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_u64(data: &[u8], offset: usize) -> io::Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "offset overflow"))?;
    if end > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PE read out of bounds",
        ));
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..end]);
    Ok(u64::from_le_bytes(buf))
}

/// Parse PE headers from raw bytes. Cross-platform (pure byte parsing).
pub fn parse_pe(data: &[u8]) -> io::Result<PeHeaders> {
    if data.len() < 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PE too small for DOS header",
        ));
    }

    let dos_magic = read_u16(data, 0)?;
    if dos_magic != DOS_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid DOS signature",
        ));
    }

    let pe_offset = read_u32(data, 60)? as usize;
    let sig = read_u32(data, pe_offset)?;
    if sig != PE_SIGNATURE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid PE signature",
        ));
    }

    let coff_offset = pe_offset + 4;
    let machine = read_u16(data, coff_offset)?;
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported PE machine type (only x64)",
        ));
    }

    let num_sections = read_u16(data, coff_offset + 2)? as usize;
    let optional_hdr_size = read_u16(data, coff_offset + 16)? as usize;
    let optional_offset = coff_offset + 20;

    if optional_hdr_size < 112 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PE optional header too small",
        ));
    }

    let opt_magic = read_u16(data, optional_offset)?;
    if opt_magic != OPTIONAL_MAGIC_PE32_PLUS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not a PE32+ (64-bit) image",
        ));
    }

    let entry_point_rva = read_u32(data, optional_offset + 16)?;
    let image_base = read_u64(data, optional_offset + 24)?;
    let section_alignment = read_u32(data, optional_offset + 32)?;
    let size_of_image = read_u32(data, optional_offset + 56)?;

    let num_data_dirs = read_u32(data, optional_offset + 108)? as usize;
    let data_dir_offset = optional_offset + 112;

    let import_dir_rva;
    let import_dir_size;
    if num_data_dirs > 1 {
        import_dir_rva = read_u32(data, data_dir_offset + 8)?;
        import_dir_size = read_u32(data, data_dir_offset + 12)?;
    } else {
        import_dir_rva = 0;
        import_dir_size = 0;
    }

    let reloc_dir_rva;
    let reloc_dir_size;
    if num_data_dirs > 5 {
        reloc_dir_rva = read_u32(data, data_dir_offset + 40)?;
        reloc_dir_size = read_u32(data, data_dir_offset + 44)?;
    } else {
        reloc_dir_rva = 0;
        reloc_dir_size = 0;
    }

    let sections_offset = optional_offset + optional_hdr_size;
    let mut sections = Vec::with_capacity(num_sections);

    if num_sections > 96 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Too many PE sections",
        ));
    }

    for i in 0..num_sections {
        let s = sections_offset + i * 40;
        let virtual_size = read_u32(data, s + 8)?;
        let virtual_address = read_u32(data, s + 12)?;
        let raw_data_size = read_u32(data, s + 16)?;
        let raw_data_offset = read_u32(data, s + 20)?;
        let characteristics = read_u32(data, s + 36)?;

        if (virtual_address as u64) + (virtual_size as u64) > size_of_image as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PE section exceeds image size",
            ));
        }

        sections.push(SectionInfo {
            virtual_address,
            virtual_size,
            raw_data_offset,
            raw_data_size,
            characteristics,
        });
    }

    Ok(PeHeaders {
        image_base,
        size_of_image,
        entry_point_rva,
        section_alignment,
        sections,
        import_dir_rva,
        import_dir_size,
        reloc_dir_rva,
        reloc_dir_size,
    })
}

/// Load and execute a PE from memory (Windows only).
#[cfg(target_os = "windows")]
pub fn load_and_exec_pe(pe_bytes: &[u8], _args: &[String]) -> io::Result<i32> {
    let headers = parse_pe(pe_bytes)?;
    let size = headers.size_of_image as usize;

    let base = unsafe {
        VirtualAlloc(
            headers.image_base as usize,
            size,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        )
    };

    let base = if base.is_null() {
        let fallback = unsafe { VirtualAlloc(0, size, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE) };
        if fallback.is_null() {
            return Err(io::Error::other("VirtualAlloc failed"));
        }
        fallback
    } else {
        base
    };

    let result = unsafe { load_pe_at(base, pe_bytes, &headers) };

    if result.is_err() {
        unsafe {
            VirtualFree(base, 0, MEM_RELEASE);
        }
    }

    result
}

#[cfg(target_os = "windows")]
unsafe fn load_pe_at(base: *mut u8, pe_bytes: &[u8], headers: &PeHeaders) -> io::Result<i32> {
    map_sections(base, pe_bytes, headers)?;
    process_relocations(base, headers)?;
    resolve_imports(base, pe_bytes, headers)?;
    set_section_protections(base, headers)?;

    FlushInstructionCache(GetCurrentProcess(), base, headers.size_of_image as usize);

    let entry = base.add(headers.entry_point_rva as usize);
    let entry_fn: extern "system" fn() -> i32 = core::mem::transmute(entry);
    Ok(entry_fn())
}

#[cfg(target_os = "windows")]
unsafe fn map_sections(base: *mut u8, pe_bytes: &[u8], headers: &PeHeaders) -> io::Result<()> {
    // Copy PE headers (up to first section or section_alignment)
    let hdr_size = std::cmp::min(headers.section_alignment as usize, pe_bytes.len());
    core::ptr::copy_nonoverlapping(pe_bytes.as_ptr(), base, hdr_size);

    for section in &headers.sections {
        let dest = base.add(section.virtual_address as usize);
        if section.raw_data_size > 0 {
            let src_start = section.raw_data_offset as usize;
            let src_end = src_start + section.raw_data_size as usize;
            if src_end > pe_bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Section raw data exceeds PE file",
                ));
            }
            core::ptr::copy_nonoverlapping(
                pe_bytes.as_ptr().add(src_start),
                dest,
                section.raw_data_size as usize,
            );
        }
        // Zero-fill the rest up to virtual_size
        let fill_start = section.raw_data_size as usize;
        let fill_end = section.virtual_size as usize;
        if fill_end > fill_start {
            core::ptr::write_bytes(dest.add(fill_start), 0, fill_end - fill_start);
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn process_relocations(base: *mut u8, headers: &PeHeaders) -> io::Result<()> {
    if headers.reloc_dir_rva == 0 || headers.reloc_dir_size == 0 {
        return Ok(());
    }

    let delta = (base as u64).wrapping_sub(headers.image_base);
    if delta == 0 {
        return Ok(());
    }

    let reloc_base = base.add(headers.reloc_dir_rva as usize);
    let mut offset: usize = 0;
    let total = headers.reloc_dir_size as usize;

    while offset + 8 <= total {
        let block_rva = *(reloc_base.add(offset) as *const u32);
        let block_size = *(reloc_base.add(offset + 4) as *const u32);
        if block_size < 8 {
            break;
        }

        let entry_count = ((block_size as usize) - 8) / 2;
        let entries_ptr = reloc_base.add(offset + 8) as *const u16;

        for i in 0..entry_count {
            let entry = *entries_ptr.add(i);
            let reloc_type = entry >> 12;
            let reloc_offset = (entry & 0x0FFF) as u32;

            match reloc_type {
                IMAGE_REL_BASED_DIR64 => {
                    let addr = base.add((block_rva + reloc_offset) as usize) as *mut u64;
                    *addr = (*addr).wrapping_add(delta);
                }
                IMAGE_REL_BASED_ABSOLUTE => {}
                _ => {}
            }
        }

        offset += block_size as usize;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn resolve_imports(base: *mut u8, _pe_bytes: &[u8], headers: &PeHeaders) -> io::Result<()> {
    if headers.import_dir_rva == 0 || headers.import_dir_size == 0 {
        return Ok(());
    }

    let import_base = base.add(headers.import_dir_rva as usize);
    let mut desc_offset: usize = 0;

    loop {
        let ilt_rva = *(import_base.add(desc_offset) as *const u32);
        let name_rva = *(import_base.add(desc_offset + 12) as *const u32);
        let iat_rva = *(import_base.add(desc_offset + 16) as *const u32);

        if ilt_rva == 0 && name_rva == 0 && iat_rva == 0 {
            break;
        }

        let dll_name_ptr = base.add(name_rva as usize);
        let dll_handle = LoadLibraryA(dll_name_ptr);
        if dll_handle == 0 {
            return Err(io::Error::other("Failed to load DLL"));
        }

        let lookup_rva = if ilt_rva != 0 { ilt_rva } else { iat_rva };
        let mut thunk_offset: usize = 0;

        loop {
            let lookup_ptr = base.add(lookup_rva as usize + thunk_offset) as *const u64;
            let thunk_data = *lookup_ptr;
            if thunk_data == 0 {
                break;
            }

            let proc_addr = if thunk_data & (1u64 << 63) != 0 {
                // Import by ordinal
                let ordinal = (thunk_data & 0xFFFF) as u16;
                GetProcAddress(dll_handle, ordinal as usize as *const u8)
            } else {
                // Import by name (skip 2-byte hint)
                let hint_name_rva = (thunk_data & 0x7FFF_FFFF) as u32;
                let func_name_ptr = base.add(hint_name_rva as usize + 2);
                GetProcAddress(dll_handle, func_name_ptr)
            };

            if proc_addr == 0 {
                return Err(io::Error::other("Failed to resolve import"));
            }

            let iat_ptr = base.add(iat_rva as usize + thunk_offset) as *mut u64;
            *iat_ptr = proc_addr as u64;

            thunk_offset += 8;
        }

        desc_offset += 20;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn section_protection(characteristics: u32) -> u32 {
    let r = characteristics & IMAGE_SCN_MEM_READ != 0;
    let w = characteristics & IMAGE_SCN_MEM_WRITE != 0;
    let x = characteristics & IMAGE_SCN_MEM_EXECUTE != 0;

    match (x, w, r) {
        (true, true, _) => PAGE_EXECUTE_READWRITE,
        (true, false, true) => PAGE_EXECUTE_READ,
        (true, false, false) => PAGE_EXECUTE,
        (false, true, _) => PAGE_READWRITE,
        (false, false, true) => PAGE_READONLY,
        (false, false, false) => PAGE_NOACCESS,
    }
}

#[cfg(target_os = "windows")]
unsafe fn set_section_protections(base: *mut u8, headers: &PeHeaders) -> io::Result<()> {
    for section in &headers.sections {
        let addr = base.add(section.virtual_address as usize);
        let size = section.virtual_size as usize;
        if size == 0 {
            continue;
        }
        let prot = section_protection(section.characteristics);
        let mut old_prot: u32 = 0;
        let ret = VirtualProtect(addr, size, prot, &mut old_prot);
        if ret == 0 {
            return Err(io::Error::other("VirtualProtect failed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid PE32+ (x64) image for testing the parser.
    fn build_minimal_pe() -> Vec<u8> {
        let mut pe = vec![0u8; 512];

        pe[0] = 0x4D;
        pe[1] = 0x5A;

        // e_lfanew at offset 60 -> points to PE signature
        let pe_offset: u32 = 128;
        pe[60..64].copy_from_slice(&pe_offset.to_le_bytes());

        let o = pe_offset as usize;
        // PE signature
        pe[o..o + 4].copy_from_slice(&PE_SIGNATURE.to_le_bytes());

        // COFF header (20 bytes at o+4)
        let coff = o + 4;
        pe[coff..coff + 2].copy_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
        // NumberOfSections = 1
        pe[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        // SizeOfOptionalHeader = 240 (standard PE32+)
        pe[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());

        // Optional header (at coff + 20)
        let opt = coff + 20;
        // Magic = PE32+
        pe[opt..opt + 2].copy_from_slice(&OPTIONAL_MAGIC_PE32_PLUS.to_le_bytes());
        // AddressOfEntryPoint (opt + 16)
        pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        // ImageBase (opt + 24)
        pe[opt + 24..opt + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes());
        // SectionAlignment (opt + 32)
        pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes());
        // FileAlignment (opt + 36)
        pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes());
        // SizeOfImage (opt + 56)
        pe[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes());
        // SizeOfHeaders (opt + 60)
        pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes());
        // NumberOfRvaAndSizes (opt + 108) = 16
        pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());

        // Section headers start at opt + 240
        let sec = opt + 240;
        // Name: ".text\0\0\0"
        pe[sec..sec + 5].copy_from_slice(b".text");
        // VirtualSize (sec + 8)
        pe[sec + 8..sec + 12].copy_from_slice(&0x100u32.to_le_bytes());
        // VirtualAddress (sec + 12)
        pe[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        // SizeOfRawData (sec + 16)
        pe[sec + 16..sec + 20].copy_from_slice(&0x100u32.to_le_bytes());
        // PointerToRawData (sec + 20)
        pe[sec + 20..sec + 24].copy_from_slice(&0x200u32.to_le_bytes());
        // Characteristics (sec + 36): MEM_EXECUTE | MEM_READ
        let chars = IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ;
        pe[sec + 36..sec + 40].copy_from_slice(&chars.to_le_bytes());

        pe
    }

    #[test]
    fn test_parse_minimal_pe() {
        let pe = build_minimal_pe();
        let headers = parse_pe(&pe).unwrap();
        assert_eq!(headers.image_base, 0x0040_0000);
        assert_eq!(headers.size_of_image, 0x3000);
        assert_eq!(headers.entry_point_rva, 0x1000);
        assert_eq!(headers.section_alignment, 0x1000);
        assert_eq!(headers.sections.len(), 1);
        assert_eq!(headers.sections[0].virtual_address, 0x1000);
        assert_eq!(headers.sections[0].virtual_size, 0x100);
    }

    #[test]
    fn test_parse_pe_section_fields() {
        let pe = build_minimal_pe();
        let headers = parse_pe(&pe).unwrap();
        let sec = &headers.sections[0];
        assert_eq!(sec.raw_data_offset, 0x200);
        assert_eq!(sec.raw_data_size, 0x100);
        let chars = IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ;
        assert_eq!(sec.characteristics, chars);
    }

    #[test]
    fn test_parse_pe_section_with_write_flag() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let opt = pe_off + 4 + 20;
        let sec = opt + 240;
        // Set characteristics to READ | WRITE (data section)
        let chars = IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE;
        pe[sec + 36..sec + 40].copy_from_slice(&chars.to_le_bytes());
        let headers = parse_pe(&pe).unwrap();
        assert_eq!(headers.sections[0].characteristics, chars);
    }

    #[test]
    fn test_parse_pe_import_reloc_dirs() {
        let pe = build_minimal_pe();
        let headers = parse_pe(&pe).unwrap();
        // Minimal PE has no imports or relocations set
        assert_eq!(headers.import_dir_rva, 0);
        assert_eq!(headers.import_dir_size, 0);
        assert_eq!(headers.reloc_dir_rva, 0);
        assert_eq!(headers.reloc_dir_size, 0);
    }

    #[test]
    fn test_sec_parse_pe_too_small() {
        let data = vec![0u8; 10];
        let result = parse_pe(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("too small"));
    }

    #[test]
    fn test_sec_parse_pe_bad_dos_magic() {
        let mut pe = build_minimal_pe();
        pe[0] = 0x00; // corrupt MZ
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("DOS signature"));
    }

    #[test]
    fn test_sec_parse_pe_bad_pe_signature() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        pe[pe_off] = 0x00; // corrupt PE signature
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("PE signature"));
    }

    #[test]
    fn test_sec_parse_pe_wrong_machine() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let coff = pe_off + 4;
        pe[coff..coff + 2].copy_from_slice(&0x014Cu16.to_le_bytes()); // i386
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("machine type"));
    }

    #[test]
    fn test_sec_parse_pe_bad_optional_magic() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let opt = pe_off + 4 + 20;
        pe[opt..opt + 2].copy_from_slice(&0x010Bu16.to_le_bytes()); // PE32
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("PE32+"));
    }

    #[test]
    fn test_sec_parse_pe_section_exceeds_image() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let opt = pe_off + 4 + 20;
        let sec = opt + 240;
        // Set VirtualSize to exceed SizeOfImage
        pe[sec + 8..sec + 12].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("exceeds image size"));
    }

    #[test]
    fn test_sec_parse_pe_too_many_sections() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let coff = pe_off + 4;
        // Set NumberOfSections to 100 (exceeds limit of 96)
        pe[coff + 2..coff + 4].copy_from_slice(&100u16.to_le_bytes());
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Too many PE sections"));
    }

    #[test]
    fn test_sec_parse_pe_optional_header_too_small() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let coff = pe_off + 4;
        // Set SizeOfOptionalHeader to 10 (too small)
        pe[coff + 16..coff + 18].copy_from_slice(&10u16.to_le_bytes());
        let result = parse_pe(&pe);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("optional header too small"));
    }

    #[test]
    fn test_sec_parse_pe_empty_input() {
        let result = parse_pe(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_parse_pe_truncated_at_pe_offset() {
        // Valid DOS header but data ends before PE signature
        let mut pe = vec![0u8; 64];
        pe[0] = 0x4D;
        pe[1] = 0x5A;
        pe[60..64].copy_from_slice(&200u32.to_le_bytes()); // offset beyond
        let result = parse_pe(&pe);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_u16_out_of_bounds() {
        let data = [0u8; 1];
        assert!(read_u16(&data, 0).is_err());
    }

    #[test]
    fn test_read_u32_out_of_bounds() {
        let data = [0u8; 3];
        assert!(read_u32(&data, 0).is_err());
    }

    #[test]
    fn test_read_u64_out_of_bounds() {
        let data = [0u8; 7];
        assert!(read_u64(&data, 0).is_err());
    }

    #[test]
    fn test_read_u16_valid() {
        let data = [0x34, 0x12];
        assert_eq!(read_u16(&data, 0).unwrap(), 0x1234);
    }

    #[test]
    fn test_read_u32_valid() {
        let data = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(read_u32(&data, 0).unwrap(), 0x1234_5678);
    }

    #[test]
    fn test_read_u64_valid() {
        let data = 0x0102_0304_0506_0708u64.to_le_bytes();
        assert_eq!(read_u64(&data, 0).unwrap(), 0x0102_0304_0506_0708);
    }

    #[test]
    fn test_read_u16_offset_overflow() {
        let data = [0u8; 4];
        assert!(read_u16(&data, usize::MAX).is_err());
    }

    #[test]
    fn test_read_u32_offset_overflow() {
        let data = [0u8; 4];
        assert!(read_u32(&data, usize::MAX).is_err());
    }

    #[test]
    fn test_read_u64_offset_overflow() {
        let data = [0u8; 8];
        assert!(read_u64(&data, usize::MAX).is_err());
    }

    #[test]
    fn test_parse_pe_no_data_dirs() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let opt = pe_off + 4 + 20;
        // Set NumberOfRvaAndSizes to 0
        pe[opt + 108..opt + 112].copy_from_slice(&0u32.to_le_bytes());
        let headers = parse_pe(&pe).unwrap();
        assert_eq!(headers.import_dir_rva, 0);
        assert_eq!(headers.reloc_dir_rva, 0);
    }

    #[test]
    fn test_parse_pe_few_data_dirs() {
        let mut pe = build_minimal_pe();
        let pe_off = u32::from_le_bytes([pe[60], pe[61], pe[62], pe[63]]) as usize;
        let opt = pe_off + 4 + 20;
        // Set NumberOfRvaAndSizes to 3 (has import dir but no reloc)
        pe[opt + 108..opt + 112].copy_from_slice(&3u32.to_le_bytes());
        let headers = parse_pe(&pe).unwrap();
        // import dir should be parsed (dir index 1)
        assert_eq!(headers.reloc_dir_rva, 0);
        assert_eq!(headers.reloc_dir_size, 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_section_protection_flags() {
        let rx = IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ;
        assert_eq!(section_protection(rx), PAGE_EXECUTE_READ);

        let rw = IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE;
        assert_eq!(section_protection(rw), PAGE_READWRITE);

        let rwx = IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE | IMAGE_SCN_MEM_EXECUTE;
        assert_eq!(section_protection(rwx), PAGE_EXECUTE_READWRITE);

        assert_eq!(section_protection(IMAGE_SCN_MEM_READ), PAGE_READONLY);

        assert_eq!(section_protection(IMAGE_SCN_MEM_EXECUTE), PAGE_EXECUTE);

        assert_eq!(section_protection(0), PAGE_NOACCESS);
    }
}
