//! Puts the app icon and version details into the Windows executable.
//!
//! Cargo knows nothing about `rc.exe`, so this script finds the resource
//! compiler in the Windows SDK, compiles `assets/app.rc` (the icon drawn by
//! `tools/make_icon.py`) and hands the resulting `.res` to the linker. That is
//! the icon Explorer, the taskbar and the title bar read from the .exe itself.
//!
//! On other systems, or without a Windows SDK, the build still succeeds and
//! only prints a warning: the window icon that `main.rs` sets at runtime works
//! either way.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Some(rc) = find_rc() else {
        println!(
            "cargo:warning=rc.exe (Windows SDK) not found, so the .exe keeps the default icon"
        );
        return;
    };

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let assets = manifest.join("assets");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("folio.res");

    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&out)
        .arg("/I")
        .arg(&assets)
        .arg(assets.join("app.rc"))
        .status();

    match status {
        Ok(status) if status.success() && out.is_file() => {
            println!("cargo:rustc-link-arg-bins={}", out.display());
        }
        Ok(status) => {
            println!("cargo:warning=rc.exe failed ({status}); the .exe keeps the default icon")
        }
        Err(error) => println!("cargo:warning=could not run {}: {error}", rc.display()),
    }
}

/// `rc.exe` comes from the Windows SDK: on PATH, in the `RC` override, or in
/// the newest versioned SDK under Program Files.
fn find_rc() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RC").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = on_path("rc.exe") {
        return Some(path);
    }

    let kits = std::env::var_os("ProgramFiles(x86)")
        .or_else(|| std::env::var_os("ProgramW6432"))
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)?;
    let bin = kits.join("Windows Kits").join("10").join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&bin)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();
    for version in versions.iter().rev() {
        for arch in ["x64", "x86", "arm64"] {
            let candidate = version.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}
