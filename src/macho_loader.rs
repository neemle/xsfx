use std::io;

// Mach-O constants
const MH_MAGIC_64: u32 = 0xFEED_FACF;
const MH_EXECUTE: u32 = 0x0000_0002;
const MH_BUNDLE: u32 = 0x0000_0008;
const FILETYPE_OFFSET: usize = 12;

#[cfg(target_os = "macos")]
const NSLINKMODULE_OPTION_PRIVATE: u32 = 0x2;

#[cfg(target_os = "macos")]
const NS_OBJECT_FILE_IMAGE_SUCCESS: i32 = 1;

#[cfg(target_os = "macos")]
extern "C" {
    fn NSCreateObjectFileImageFromMemory(
        addr: *const u8,
        size: usize,
        image: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn NSLinkModule(
        image: *mut core::ffi::c_void,
        name: *const u8,
        options: u32,
    ) -> *mut core::ffi::c_void;
    fn NSLookupSymbolInModule(
        module: *mut core::ffi::c_void,
        name: *const u8,
    ) -> *mut core::ffi::c_void;
    fn NSAddressOfSymbol(symbol: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}

/// Validate Mach-O magic and return the file type at offset 12.
pub fn validate_macho(data: &[u8]) -> io::Result<u32> {
    if data.len() < 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Mach-O too small for header",
        ));
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != MH_MAGIC_64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Mach-O magic",
        ));
    }

    let filetype = u32::from_le_bytes([
        data[FILETYPE_OFFSET],
        data[FILETYPE_OFFSET + 1],
        data[FILETYPE_OFFSET + 2],
        data[FILETYPE_OFFSET + 3],
    ]);

    Ok(filetype)
}

/// Patch MH_EXECUTE to MH_BUNDLE in a mutable copy of Mach-O bytes.
/// Returns the patched buffer.
pub fn patch_filetype_to_bundle(data: &[u8]) -> io::Result<Vec<u8>> {
    let filetype = validate_macho(data)?;
    if filetype != MH_EXECUTE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Mach-O is not MH_EXECUTE",
        ));
    }

    let mut patched = data.to_vec();
    patched[FILETYPE_OFFSET..FILETYPE_OFFSET + 4].copy_from_slice(&MH_BUNDLE.to_le_bytes());
    Ok(patched)
}

/// Load and execute a Mach-O binary from memory (macOS only).
#[cfg(target_os = "macos")]
pub fn load_and_exec_macho(macho_bytes: &[u8], args: &[String]) -> io::Result<i32> {
    let patched = patch_filetype_to_bundle(macho_bytes)?;

    let mut image: *mut core::ffi::c_void = core::ptr::null_mut();
    let result =
        unsafe { NSCreateObjectFileImageFromMemory(patched.as_ptr(), patched.len(), &mut image) };
    if result != NS_OBJECT_FILE_IMAGE_SUCCESS {
        return Err(io::Error::other("Failed to create object file image"));
    }

    let module_name = b"payload\0";
    let module = unsafe { NSLinkModule(image, module_name.as_ptr(), NSLINKMODULE_OPTION_PRIVATE) };
    if module.is_null() {
        return Err(io::Error::other("Failed to link module"));
    }

    let sym_name = b"_main\0";
    let symbol = unsafe { NSLookupSymbolInModule(module, sym_name.as_ptr()) };
    if symbol.is_null() {
        return Err(io::Error::other("Failed to find _main symbol"));
    }

    let addr = unsafe { NSAddressOfSymbol(symbol) };
    if addr.is_null() {
        return Err(io::Error::other("Failed to get address of _main"));
    }

    let main_fn: extern "C" fn(i32, *const *const u8) -> i32 =
        unsafe { core::mem::transmute(addr) };

    let c_args = build_c_args(args);
    let c_ptrs: Vec<*const u8> = c_args.iter().map(|a| a.as_ptr()).collect();

    let exit_code = main_fn(c_ptrs.len() as i32, c_ptrs.as_ptr());
    Ok(exit_code)
}

#[cfg(target_os = "macos")]
fn build_c_args(args: &[String]) -> Vec<Vec<u8>> {
    args.iter()
        .map(|a| {
            let mut v = a.as_bytes().to_vec();
            v.push(0);
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_minimal_macho_execute() -> Vec<u8> {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&MH_MAGIC_64.to_le_bytes());
        // cputype at offset 4 (don't care for validation)
        // cpusubtype at offset 8 (don't care)
        // filetype at offset 12 = MH_EXECUTE
        data[12..16].copy_from_slice(&MH_EXECUTE.to_le_bytes());
        data
    }

    #[test]
    fn test_validate_macho_valid() {
        let data = build_minimal_macho_execute();
        let filetype = validate_macho(&data).unwrap();
        assert_eq!(filetype, MH_EXECUTE);
    }

    #[test]
    fn test_patch_filetype_to_bundle() {
        let data = build_minimal_macho_execute();
        let patched = patch_filetype_to_bundle(&data).unwrap();

        let new_filetype = u32::from_le_bytes([patched[12], patched[13], patched[14], patched[15]]);
        assert_eq!(new_filetype, MH_BUNDLE);

        // Magic should be unchanged
        let magic = u32::from_le_bytes([patched[0], patched[1], patched[2], patched[3]]);
        assert_eq!(magic, MH_MAGIC_64);
    }

    #[test]
    fn test_patch_preserves_other_bytes() {
        let mut data = build_minimal_macho_execute();
        data[4..8].copy_from_slice(&0x0100_000Cu32.to_le_bytes()); // cputype
        data[16..20].copy_from_slice(&42u32.to_le_bytes()); // ncmds

        let patched = patch_filetype_to_bundle(&data).unwrap();
        // cputype preserved
        assert_eq!(
            u32::from_le_bytes([patched[4], patched[5], patched[6], patched[7]]),
            0x0100_000C
        );
        // ncmds preserved
        assert_eq!(
            u32::from_le_bytes([patched[16], patched[17], patched[18], patched[19]]),
            42
        );
    }

    #[test]
    fn test_sec_uc002_validate_macho_too_small() {
        let data = vec![0u8; 8];
        let result = validate_macho(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("too small"));
    }

    #[test]
    fn test_sec_uc002_validate_macho_bad_magic() {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let result = validate_macho(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid Mach-O magic"));
    }

    #[test]
    fn test_sec_uc002_validate_macho_empty() {
        let result = validate_macho(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_uc002_patch_not_execute() {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&MH_MAGIC_64.to_le_bytes());
        data[12..16].copy_from_slice(&MH_BUNDLE.to_le_bytes());
        let result = patch_filetype_to_bundle(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not MH_EXECUTE"));
    }

    #[test]
    fn test_sec_uc002_patch_bad_magic() {
        let data = vec![0xFFu8; 32];
        let result = patch_filetype_to_bundle(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_sec_uc002_patch_truncated() {
        let result = patch_filetype_to_bundle(&[0xCF, 0xFA, 0xED, 0xFE]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_macho_32bit_magic_rejected() {
        let mut data = vec![0u8; 32];
        // MH_MAGIC (32-bit) = 0xFEEDFACE
        data[0..4].copy_from_slice(&0xFEED_FACEu32.to_le_bytes());
        let result = validate_macho(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_patch_returns_new_vec() {
        let original = build_minimal_macho_execute();
        let patched = patch_filetype_to_bundle(&original).unwrap();
        // Original should be unchanged
        let orig_filetype =
            u32::from_le_bytes([original[12], original[13], original[14], original[15]]);
        assert_eq!(orig_filetype, MH_EXECUTE);
        assert_ne!(original, patched);
    }
}
