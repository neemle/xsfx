use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::PathBuf;

use lzma_rs::xz_compress;

use xsfx::common::Trailer;

#[cfg(feature = "native-compress")]
use xz2::write::XzEncoder;

// Include the generated catalog of all built stubs.
// This is produced by build.rs in OUT_DIR as stub_catalog.rs
mod stub_catalog {
    include!(concat!(env!("OUT_DIR"), "/stub_catalog.rs"));
}


fn main() -> io::Result<()> {
    let mut args = env::args().collect::<Vec<String>>();

    // Simple CLI: xsfx <input_payload> <output_sfx> [--target <triple>]
    if args.len() < 3 || args.len() > 5 {
        eprintln!(
            "Usage: {} <input_payload> <output_sfx> [--target <triple>]",
            args[0]
        );
        list_available_stubs();
        std::process::exit(1);
    }

    let payload_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);

    // Parse optional --target
    let mut selected_target: Option<String> = None;
    if args.len() > 3 {
        if args.len() == 5 && args[3] == "--target" {
            selected_target = Some(args[4].clone());
        } else {
            eprintln!(
                "Unknown arguments. Usage: {} <input_payload> <output_sfx> [--target <triple>]",
                args[0]
            );
            list_available_stubs();
            std::process::exit(1);
        }
    }

    let target_to_use = selected_target
        .or_else(|| env::var("XSFX_OUT_TARGET").ok())
        .unwrap_or_else(|| stub_catalog::DEFAULT_TARGET.to_string());

    let stub_bytes = match find_stub(&target_to_use) {
        Some(bytes) => bytes,
        None => {
            eprintln!(
                "Requested target '{}' not available in this build.",
                target_to_use
            );
            list_available_stubs();
            std::process::exit(2);
        }
    };

    // Read payload (the app to pack)
    let payload_bytes = fs::read(&payload_path).map_err(|e| {
        eprintln!("Failed to read payload {}: {}", payload_path.display(), e);
        e
    })?;

    // Compress payload using LZMA (lzma-rs)
    let compressed_payload = compress_lzma(&payload_bytes)?;

    let payload_len = compressed_payload.len() as u64;
    let trailer = Trailer::new(payload_len);
    let trailer_bytes = trailer.to_bytes();

    // Write out final SFX: [stub][compressed payload][trailer]
    let mut out = File::create(&output_path).map_err(|e| {
        eprintln!("Failed to create output {}: {}", output_path.display(), e);
        e
    })?;

    out.write_all(&stub_bytes)?;
    out.write_all(&compressed_payload)?;
    out.write_all(&trailer_bytes)?;
    out.flush()?;

    println!(
        "Created SFX: {} (target: {}, stub: {} bytes, payload: {} bytes compressed)",
        output_path.display(),
        target_to_use,
        stub_bytes.len(),
        payload_len
    );

    Ok(())
}

fn compress_lzma(data: &[u8]) -> io::Result<Vec<u8>> {
    // Prefer native liblzma (xz2) when available; fallback to pure-Rust lzma-rs.
    #[cfg(feature = "native-compress")]
    {
        let mut encoder = XzEncoder::new(Vec::new(), 9); // level 9 = max compression
        encoder.write_all(data)?;
        encoder.flush()?;
        let compressed = encoder.finish()?;
        return Ok(compressed);
    }

    let mut reader = BufReader::new(io::Cursor::new(data));
    let mut compressed = Vec::new();

    // lzma-rs expects a BufRead; it uses default compression options internally.
    xz_compress(&mut reader, &mut compressed)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(compressed)
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
        eprintln!("  - {}{}", entry.target, if entry.target == stub_catalog::DEFAULT_TARGET { " (default)" } else { "" });
    }
    eprintln!("\nTo add more, set XSFX_TARGETS=comma,separated,triples before building.");
}
