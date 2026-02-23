use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// Multi-stub catalog generation.
///
/// By default builds a stub only for the current host target.
/// Override via env vars:
///   XSFX_TARGETS=all              → build for all common targets
///   XSFX_TARGETS=t1,t2            → build for specific targets
///   XSFX_PREBUILT_STUBS_DIR=path  → use pre-built stubs instead of building
///   XSFX_SKIP_STUB_BUILD=1        → generate empty catalog (for tests/clippy)
const ALL_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/bin/stub.rs");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=XSFX_TARGETS");
    println!("cargo:rerun-if-env-changed=XSFX_TARGET");
    println!("cargo:rerun-if-env-changed=XSFX_PREBUILT_STUBS_DIR");
    println!("cargo:rerun-if-env-changed=XSFX_SKIP_STUB_BUILD");

    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("stub_catalog.rs");

    // Skip stub building (used by test/clippy stages that only compile the library)
    if env::var("XSFX_SKIP_STUB_BUILD").is_ok() {
        write_stub_catalog(&out_path, &[])?;
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));
    let stub_target_dir = target_dir.join("stubs");

    let host_target = env::var("TARGET").unwrap_or_else(|_| "x86_64-unknown-linux-gnu".into());

    let targets = resolve_targets(&host_target);

    println!("cargo:warning=xsfx build.rs: targets={}", targets.join(","));

    let mut built_stubs = Vec::new();

    if let Some(dir) = env::var_os("XSFX_PREBUILT_STUBS_DIR").map(PathBuf::from) {
        println!("cargo:warning=Using prebuilt stubs from {}", dir.display());
        for target in &targets {
            let file = format!("stub{}", exe_suffix(target));
            let path = dir.join(target).join(&file);
            if path.exists() {
                built_stubs.push((target.clone(), path));
            } else {
                let alt = dir.join(format!("{}-{}", target, file));
                if alt.exists() {
                    built_stubs.push((target.clone(), alt));
                } else {
                    return Err(format!(
                        "prebuilt stub for {} not found at {} or {}",
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
                    println!("cargo:warning=Skipping stub {}: {}", target, err);
                }
            }
        }
        if built_stubs.is_empty() {
            return Err("no stubs built; set XSFX_TARGETS or XSFX_PREBUILT_STUBS_DIR".into());
        }
    }

    println!(
        "cargo:warning=Generating stub catalog ({} entries)",
        built_stubs.len()
    );
    write_stub_catalog(&out_path, &built_stubs)?;

    Ok(())
}

fn resolve_targets(host_target: &str) -> Vec<String> {
    let raw = env::var("XSFX_TARGETS")
        .or_else(|_| env::var("XSFX_TARGET"))
        .ok();

    match raw {
        Some(val) => {
            let trimmed = val.trim().to_ascii_lowercase();
            if trimmed.is_empty() || trimmed == "all" {
                ALL_TARGETS.iter().map(|s| s.to_string()).collect()
            } else {
                val.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
        }
        None => vec![host_target.to_string()],
    }
}

fn build_stub(target: &str, target_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO")?;
    let mut cmd = Command::new(cargo);
    cmd.env("XSFX_SKIP_STUB_BUILD", "1");
    cmd.args([
        "build",
        "--no-default-features",
        "--bin",
        "stub",
        "--release",
        "--target",
        target,
    ]);
    cmd.arg("--target-dir").arg(target_dir);

    println!(
        "cargo:warning=Invoking cargo for stub {}: {:?}",
        target, cmd
    );

    let start = Instant::now();
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let tag = String::from(target);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let t1 = stdout.map(|out| {
        let tag = tag.clone();
        std::thread::spawn(move || {
            let reader = io::BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                println!("cargo:warning=[stub {} stdout] {}", tag, line);
            }
        })
    });

    let t2 = stderr.map(|err| {
        let tag = tag.clone();
        std::thread::spawn(move || {
            let reader = io::BufReader::new(err);
            for line in reader.lines().map_while(Result::ok) {
                println!("cargo:warning=[stub {} stderr] {}", tag, line);
            }
        })
    });

    let status = child.wait()?;
    if let Some(h) = t1 {
        let _ = h.join();
    }
    if let Some(h) = t2 {
        let _ = h.join();
    }

    println!(
        "cargo:warning=Stub {} finished {:?} in {:.2?}",
        target,
        status.code(),
        start.elapsed()
    );

    if !status.success() {
        return Err(
            format!("failed to build stub for {target}; run `rustup target add {target}`").into(),
        );
    }

    let exe = format!("stub{}", exe_suffix(target));
    let path = target_dir.join(target).join("release").join(exe);
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("stub for {target} not found at {}", path.display()),
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
        "pub struct StubEntry {{ pub target: &'static str, pub bytes: &'static [u8] }}"
    )?;
    writeln!(
        file,
        "pub const DEFAULT_TARGET: &str = \"{}\";",
        env::var("TARGET").unwrap_or_default()
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
