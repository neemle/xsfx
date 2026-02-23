use std::env;
#[cfg(target_os = "linux")]
use std::ffi::CString;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use xsfx::common::{Trailer, MAGIC, TRAILER_SIZE};
use xsfx::decompress::decompress_payload;

fn main() {
    if run_stub().is_err() {
        let _ = io::Write::write_all(&mut io::stderr(), b"SFX stub error\n");
        std::process::exit(1);
    }
}

fn read_and_validate_trailer(file: &mut std::fs::File) -> io::Result<(u64, u64)> {
    let total_len = file.metadata()?.len();
    if total_len < TRAILER_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File too small to contain trailer",
        ));
    }
    file.seek(SeekFrom::Start(total_len - TRAILER_SIZE))?;
    let trailer = Trailer::from_reader(file)?;
    if trailer.magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid SFX magic marker",
        ));
    }
    if trailer.payload_len == 0 || trailer.payload_len > total_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid payload length in trailer",
        ));
    }
    Ok((trailer.payload_len, total_len))
}

fn run_stub() -> io::Result<()> {
    let exe_path = env::current_exe()?;
    #[cfg(target_os = "linux")]
    let mut file = std::fs::File::open("/proc/self/exe")?;
    #[cfg(not(target_os = "linux"))]
    let mut file = std::fs::File::open(&exe_path)?;
    let (payload_len, total_len) = read_and_validate_trailer(&mut file)?;
    file.seek(SeekFrom::Start(total_len - TRAILER_SIZE - payload_len))?;
    let mut limited_reader = BufReader::new(file.take(payload_len));
    let payload = decompress_payload(&mut limited_reader)?;
    let args: Vec<String> = env::args().skip(1).collect();
    let exit_code = exec_payload(&payload, &args, &exe_path)?;
    std::process::exit(exit_code);
}

#[cfg(target_os = "linux")]
fn write_memfd(data: &[u8]) -> io::Result<std::fs::File> {
    use std::io::Write;
    use std::os::unix::io::{AsRawFd, FromRawFd};

    let fd = unsafe {
        let r = libc::syscall(libc::SYS_memfd_create, c"rsfx".as_ptr(), libc::MFD_CLOEXEC);
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        r as i32
    };
    let mut f = unsafe { std::fs::File::from_raw_fd(fd) };
    f.write_all(data)?;
    if unsafe { libc::fchmod(f.as_raw_fd(), 0o700) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(f)
}

#[cfg(target_os = "linux")]
fn build_c_argv(argv0: &Path, args: &[String]) -> io::Result<(CString, Vec<CString>)> {
    use std::os::unix::ffi::OsStrExt;
    let c_argv0 = CString::new(argv0.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let c_args: Vec<CString> = args
        .iter()
        .map(|a| CString::new(a.as_bytes()))
        .collect::<Result<_, _>>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((c_argv0, c_args))
}

#[cfg(target_os = "linux")]
fn exec_payload(payload: &[u8], args: &[String], argv0: &Path) -> io::Result<i32> {
    use std::os::unix::io::AsRawFd;
    extern "C" {
        static environ: *const *const libc::c_char;
    }
    let memfd = write_memfd(payload)?;
    let (c_argv0, c_args) = build_c_argv(argv0, args)?;
    let mut argv: Vec<*const libc::c_char> = Vec::with_capacity(args.len() + 2);
    argv.push(c_argv0.as_ptr());
    for a in &c_args {
        argv.push(a.as_ptr());
    }
    argv.push(std::ptr::null());
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            memfd.as_raw_fd(),
            c"".as_ptr(),
            argv.as_ptr(),
            environ,
            libc::AT_EMPTY_PATH,
        );
    }
    Err(io::Error::last_os_error())
}

#[cfg(target_os = "windows")]
fn exec_payload(payload: &[u8], args: &[String], _argv0: &Path) -> io::Result<i32> {
    xsfx::pe_loader::load_and_exec_pe(payload, args)
}

#[cfg(target_os = "macos")]
fn exec_payload(payload: &[u8], args: &[String], _argv0: &Path) -> io::Result<i32> {
    xsfx::macho_loader::load_and_exec_macho(payload, args)
}
