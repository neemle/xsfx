use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use xsfx::common::Trailer;
use xsfx::compress::compress_lzma;

mod stub_catalog {
    include!(concat!(env!("OUT_DIR"), "/stub_catalog.rs"));
}

struct PackerArgs {
    payload_path: PathBuf,
    output_path: PathBuf,
    target: String,
}

fn parse_args() -> PackerArgs {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        eprintln!("Usage: {} <input_payload> <output_sfx> [--target <triple>]", args[0]);
        list_available_stubs();
        std::process::exit(1);
    }
    let mut selected_target: Option<String> = None;
    if args.len() > 3 {
        if args.len() == 5 && args[3] == "--target" {
            selected_target = Some(args[4].clone());
        } else {
            eprintln!("Unknown arguments. Usage: {} <input_payload> <output_sfx> [--target <triple>]", args[0]);
            list_available_stubs();
            std::process::exit(1);
        }
    }
    let target = selected_target
        .or_else(|| env::var("XSFX_OUT_TARGET").ok())
        .unwrap_or_else(|| stub_catalog::DEFAULT_TARGET.to_string());
    PackerArgs {
        payload_path: PathBuf::from(&args[1]),
        output_path: PathBuf::from(&args[2]),
        target,
    }
}

fn write_sfx(stub: &[u8], payload: &[u8], output: &Path) -> io::Result<u64> {
    let compressed = compress_lzma(payload)?;
    let compressed_len = compressed.len() as u64;
    let trailer = Trailer::new(compressed_len);
    let mut out = File::create(output).map_err(|e| {
        eprintln!("Failed to create output {}: {}", output.display(), e);
        e
    })?;
    out.write_all(stub)?;
    out.write_all(&compressed)?;
    out.write_all(&trailer.to_bytes())?;
    out.flush()?;
    Ok(compressed_len)
}

fn main() -> io::Result<()> {
    let args = parse_args();
    let stub_bytes = match find_stub(&args.target) {
        Some(bytes) => bytes,
        None => {
            eprintln!("Requested target '{}' not available in this build.", args.target);
            list_available_stubs();
            std::process::exit(2);
        }
    };
    let payload_bytes = fs::read(&args.payload_path).map_err(|e| {
        eprintln!("Failed to read payload {}: {}", args.payload_path.display(), e);
        e
    })?;
    let compressed_len = write_sfx(stub_bytes, &payload_bytes, &args.output_path)?;
    println!(
        "Created SFX: {} (target: {}, stub: {} bytes, payload: {} bytes compressed)",
        args.output_path.display(), args.target, stub_bytes.len(), compressed_len
    );
    Ok(())
}

fn find_stub(target: &str) -> Option<&'static [u8]> {
    for entry in stub_catalog::STUBS {
        if entry.target == target {
            return Some(entry.bytes);
        }
    }
    None
}

fn list_available_stubs() {
    eprintln!("Available stub targets in this build:");
    for entry in stub_catalog::STUBS {
        let suffix = if entry.target == stub_catalog::DEFAULT_TARGET {
            " (default)"
        } else {
            ""
        };
        eprintln!("  - {}{}", entry.target, suffix);
    }
}
