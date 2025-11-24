use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

// Stub catalog selection
// ----------------------
// By default we build a FULL multi-stub catalog (common Linux/macOS/Windows
// targets). You can override which stubs to embed via either of the env vars:
//   - XSFX_TARGETS  (preferred)
//   - XSFX_TARGET   (alias; a comma-separated list or the word "all")
// If neither is set, we default to the full common set.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Avoid infinite recursion when this build script spawns `cargo build --bin stub`.
    if env::var("XSFX_SKIP_STUB_BUILD").is_ok() {
        println!("cargo:rerun-if-changed=src/bin/stub.rs");
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=XSFX_TARGETS");
        println!("cargo:rerun-if-env-changed=XSFX_TARGET");
        return Ok(());
    }

    println!("cargo:rerun-if-changed=src/bin/stub.rs");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=XSFX_TARGETS");
    println!("cargo:rerun-if-env-changed=XSFX_TARGET");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));
    // Use a separate target dir for the stub builds to avoid Cargo locking
    // contention or cyclic waits when the parent build is compiling the same
    // crate. Building the stub in a distinct directory breaks the lock cycle
    // and prevents the appearance of being "stuck" at xsfx(build).
    let stub_target_dir = target_dir.join("stubs");

    let default_full: Vec<String> = vec![
        // Linux GNU
        "x86_64-unknown-linux-gnu".into(),
        "aarch64-unknown-linux-gnu".into(),
        // Linux MUSL
        "x86_64-unknown-linux-musl".into(),
        "aarch64-unknown-linux-musl".into(),
        // macOS
        "x86_64-apple-darwin".into(),
        "aarch64-apple-darwin".into(),
        // Windows GNU
        "x86_64-pc-windows-gnu".into(),
        // Windows MSVC
        "x86_64-pc-windows-msvc".into(),
        "aarch64-pc-windows-msvc".into(),
    ];

    // Helper to parse a comma-separated list; returns None if empty/whitespace.
    fn parse_list(val: String) -> Option<Vec<String>> {
        let items: Vec<String> = val
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();
        if items.is_empty() { None } else { Some(items) }
    }

    let targets = if let Ok(list) = env::var("XSFX_TARGETS") {
        let lowered = list.trim().to_ascii_lowercase();
        if lowered.is_empty() || lowered == "all" {
            default_full.clone()
        } else {
            parse_list(list).unwrap_or_else(|| default_full.clone())
        }
    } else if let Ok(list) = env::var("XSFX_TARGET") {
        let lowered = list.trim().to_ascii_lowercase();
        if lowered.is_empty() || lowered == "all" {
            default_full.clone()
        } else {
            parse_list(list).unwrap_or_else(|| default_full.clone())
        }
    } else {
        default_full.clone()
    };

    println!(
        "cargo:warning=xsfx build script start: manifest_dir={}, target_dir={}, targets={}",
        manifest_dir.display(),
        target_dir.display(),
        targets.join(",")
    );

    // If user provides a directory of prebuilt stubs, consume those instead of cross-building.
    let prebuilt_dir = env::var_os("XSFX_PREBUILT_STUBS_DIR").map(PathBuf::from);
    let mut built_stubs = Vec::new();
    if let Some(dir) = prebuilt_dir {
        println!(
            "cargo:warning=Using prebuilt stubs from {}",
            dir.display()
        );
        for target in &targets {
            let file = format!("stub{}", exe_suffix(target));
            let path = dir.join(target).join(&file);
            if path.exists() {
                built_stubs.push((target.clone(), path));
            } else {
                // Also try flat layout: <dir>/<target>-<file>
                let alt = dir.join(format!("{}-{}", target, file));
                if alt.exists() {
                    built_stubs.push((target.clone(), alt));
                } else {
                    return Err(format!(
                        "prebuilt stub for {} not found (expected {} or {})",
                        target,
                        path.display(),
                        alt.display()
                    )
                    .into());
                }
            }
        }
    } else {
        let total = targets.len();
        for (idx, target) in targets.into_iter().enumerate() {
            println!(
                "cargo:warning=Step {}/{}: building stub for {}",
                idx + 1,
                total,
                target
            );
            match build_stub(&target, &stub_target_dir) {
                Ok(path) => {
                    println!(
                        "cargo:warning=Step {}/{}: finished stub for {} at {}",
                        idx + 1,
                        total,
                        target,
                        path.display()
                    );
                    built_stubs.push((target, path));
                }
                Err(err) => {
                    println!(
                        "cargo:warning=Skipping stub {}: {}",
                        target,
                        err
                    );
                }
            }
        }
        if built_stubs.is_empty() {
            return Err("no stubs were built successfully; provide XSFX_PREBUILT_STUBS_DIR or reduce XSFX_TARGETS".into());
        }
    }

    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("stub_catalog.rs");
    println!(
        "cargo:warning=Generating stub catalog at {} ({} entries)",
        out_path.display(),
        built_stubs.len()
    );
    write_stub_catalog(&out_path, &built_stubs)?;
    println!("cargo:warning=Stub catalog generated.");

    Ok(())
}

fn build_stub(target: &str, target_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO")?;
    let mut cmd = Command::new(cargo);
    cmd.env("XSFX_SKIP_STUB_BUILD", "1");
    cmd.args(["build", "--bin", "stub", "--release", "--target", target]);

    // Always direct stub builds into a dedicated subdirectory to avoid build
    // directory locks with the parent build (which is compiling the same
    // crate). This prevents deadlocks when Cargo tries to concurrently build
    // the same package graph in the same target dir.
    cmd.arg("--target-dir").arg(target_dir);

    println!("cargo:warning=Invoking cargo for stub {}: {:?}", target, cmd);

    // Stream child cargo output as cargo:warning lines so progress is visible.
    let start = Instant::now();
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let tag = String::from(target);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let t1 = if let Some(out) = stdout {
        let tag = tag.clone();
        Some(std::thread::spawn(move || {
            let reader = std::io::BufReader::new(out);
            for line in reader.lines() {
                if let Ok(l) = line {
                    println!("cargo:warning=[stub {} stdout] {}", tag, l);
                }
            }
        }))
    } else { None };

    let t2 = if let Some(err) = stderr {
        let tag = tag.clone();
        Some(std::thread::spawn(move || {
            let reader = std::io::BufReader::new(err);
            for line in reader.lines() {
                if let Ok(l) = line {
                    println!("cargo:warning=[stub {} stderr] {}", tag, l);
                }
            }
        }))
    } else { None };

    let status = child.wait()?;
    if let Some(h) = t1 { let _ = h.join(); }
    if let Some(h) = t2 { let _ = h.join(); }

    println!(
        "cargo:warning=Stub {} build finished with status {:?} in {:.2?}",
        target,
        status.code(),
        start.elapsed()
    );
    if !status.success() {
        return Err(format!(
            "failed to build stub for {target}; make sure `rustup target add {target}` and any required linkers are installed"
        )
        .into());
    }

    let exe = format!("stub{}", exe_suffix(target));
    let path = target_dir.join(target).join("release").join(exe);
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("stub artifact for {target} not found at {}", path.display()),
        )
        .into());
    }

    Ok(path)
}

fn write_stub_catalog(
    out_path: &Path,
    stubs: &[(String, PathBuf)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(out_path)?;
    writeln!(
        file,
        "pub struct StubEntry {{ pub target: &'static str, pub bytes: &'static [u8], }}"
    )?;
    writeln!(
        file,
        "pub const DEFAULT_TARGET: &str = \"{}\";",
        env::var("TARGET")?
    )?;
    writeln!(file, "pub static STUBS: &[StubEntry] = &[")?;
    for (target, path) in stubs {
        let canonical = fs::canonicalize(path)?;
        writeln!(
            file,
            "    StubEntry {{ target: \"{target}\", bytes: include_bytes!(r#\"{}\"#) }},",
            canonical.display()
        )?;
    }
    writeln!(file, "];")?;
    Ok(())
}

fn exe_suffix(target: &str) -> &str {
    if target.contains("windows") {
        ".exe"
    } else {
        ""
    }
}
