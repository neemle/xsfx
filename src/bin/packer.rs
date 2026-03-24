use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};

use xsfx::common::Trailer;
use xsfx::compress::compress_lzma;

mod stub_catalog {
    include!(concat!(env!("OUT_DIR"), "/stub_catalog.rs"));
}

struct PackerArgs {
    payload_path: String,
    output_path: String,
    target: String,
}

fn print_usage(prog: &str) {
    eprintln!("Usage: {} <input> <output> [--target <triple>]", prog);
    eprintln!("  Use '-' for input to read from stdin, '-' for output to write to stdout.");
}

fn parse_args() -> PackerArgs {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        print_usage(&args[0]);
        list_available_stubs();
        std::process::exit(1);
    }
    let mut selected_target: Option<String> = None;
    if args.len() > 3 {
        if args.len() == 5 && args[3] == "--target" {
            selected_target = Some(args[4].clone());
        } else {
            print_usage(&args[0]);
            list_available_stubs();
            std::process::exit(1);
        }
    }
    let target = selected_target
        .or_else(|| env::var("XSFX_OUT_TARGET").ok())
        .unwrap_or_else(|| stub_catalog::DEFAULT_TARGET.to_string());
    PackerArgs {
        payload_path: args[1].clone(),
        output_path: args[2].clone(),
        target,
    }
}

fn read_payload(path: &str) -> io::Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().lock().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        fs::read(path).map_err(|e| {
            eprintln!("Failed to read payload {}: {}", path, e);
            e
        })
    }
}

fn open_output(path: &str) -> io::Result<Box<dyn Write>> {
    if path == "-" {
        Ok(Box::new(io::stdout().lock()))
    } else {
        let f = File::create(path).map_err(|e| {
            eprintln!("Failed to create output {}: {}", path, e);
            e
        })?;
        Ok(Box::new(f))
    }
}

fn write_sfx(stub: &[u8], payload: &[u8], writer: &mut dyn Write) -> io::Result<u64> {
    let compressed = compress_lzma(payload)?;
    let compressed_len = compressed.len() as u64;
    let trailer = Trailer::new(compressed_len);
    writer.write_all(stub)?;
    writer.write_all(&compressed)?;
    writer.write_all(&trailer.to_bytes())?;
    writer.flush()?;
    Ok(compressed_len)
}

fn main() -> io::Result<()> {
    let args = parse_args();
    let stub_bytes = match find_stub(&args.target) {
        Some(bytes) => bytes,
        None => {
            eprintln!(
                "Requested target '{}' not available in this build.",
                args.target
            );
            list_available_stubs();
            std::process::exit(2);
        }
    };
    let payload_bytes = read_payload(&args.payload_path)?;
    let mut out = open_output(&args.output_path)?;
    let compressed_len = write_sfx(stub_bytes, &payload_bytes, &mut *out)?;
    if args.output_path != "-" {
        eprintln!(
            "Created SFX: {} (target: {}, stub: {} bytes, payload: {} bytes compressed)",
            args.output_path,
            args.target,
            stub_bytes.len(),
            compressed_len
        );
    }
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
