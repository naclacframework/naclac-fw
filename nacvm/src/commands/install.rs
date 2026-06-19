use crate::config::Config;
use std::process::Command;
use std::fs;
use std::env;
use colored::*;

pub fn execute(config: &Config, version: &str) {
    let mut resolved_version = version.to_string();

    if version.to_lowercase() == "latest" {
        println!("🔍 Resolving latest version from crates.io...");
        if let Some(v) = crate::utils::resolve::get_latest_version() {
            resolved_version = v;
            println!("📦 Found latest version: v{}", resolved_version);
        }
        
        if resolved_version.to_lowercase() == "latest" {
            eprintln!("{} Failed to parse the latest version from crates.io.", "Error:".red().bold());
            std::process::exit(1);
        }
    }

    println!("{} naclac v{}...", "Installing".green().bold(), resolved_version);

    let root_path = config.versions_dir.join(&resolved_version);
    let bin_path = root_path.join("bin");

    if let Err(e) = fs::create_dir_all(&bin_path) {
        eprintln!("{} Failed to create bin directory: {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }

    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    let target = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => {
            eprintln!("{} Unsupported platform: {} - {}", "Error:".red().bold(), os, arch);
            let _ = fs::remove_dir_all(&root_path);
            std::process::exit(1);
        }
    };

    let extension = if os == "windows" { "zip" } else { "tar.gz" };
    let download_url = format!(
        "https://github.com/naclacframework/naclac-fw/releases/download/v{}/naclac-{}.{}",
        resolved_version, target, extension
    );

    let temp_archive = env::temp_dir().join(format!("naclac-{}-{}.{}", resolved_version, target, extension));
    
    println!("📥 Downloading pre-compiled binary from GitHub...");

    let download_status = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-Command",
                &format!("$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri '{}' -OutFile '{}'", download_url, temp_archive.to_string_lossy()),
            ])
            .status()
    } else {
        Command::new("curl")
            .args(&[
                "-fL",
                "-o",
                &temp_archive.to_string_lossy(),
                &download_url,
            ])
            .status()
    };

    if download_status.is_err() || !download_status.unwrap().success() {
        eprintln!("{} Failed to download pre-compiled binary.", "Error:".red().bold());
        let _ = fs::remove_dir_all(&root_path);
        let _ = fs::remove_file(&temp_archive);
        std::process::exit(1);
    }

    println!("🚚 Extracting binary...");

    let extract_status = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-Command",
                &format!("Expand-Archive -Path '{}' -DestinationPath '{}' -Force", temp_archive.to_string_lossy(), bin_path.to_string_lossy()),
            ])
            .status()
    } else {
        Command::new("tar")
            .args(&[
                "-xzf",
                &temp_archive.to_string_lossy(),
                "-C",
                &bin_path.to_string_lossy(),
            ])
            .status()
    };

    let _ = fs::remove_file(&temp_archive);

    if extract_status.is_err() || !extract_status.unwrap().success() {
        eprintln!("{} Failed to extract binary.", "Error:".red().bold());
        let _ = fs::remove_dir_all(&root_path);
        std::process::exit(1);
    }

    let exe_name = format!("naclac{}", env::consts::EXE_SUFFIX);
    let expected_bin_path = bin_path.join(&exe_name);

    if expected_bin_path.exists() {
        println!("{} v{} installed successfully at {:?}", "Version".green().bold(), resolved_version, root_path);
    } else {
        eprintln!("{} Expected binary {:?} was not found after extraction.", "Error:".red().bold(), expected_bin_path);
        let _ = fs::remove_dir_all(&root_path);
        std::process::exit(1);
    }
}
