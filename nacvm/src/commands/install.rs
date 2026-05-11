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

    println!("{} naclac v{}... (This may take a few minutes)", "Installing".green().bold(), resolved_version);

    let root_path = config.versions_dir.join(&resolved_version);
    let root_str = root_path.to_string_lossy();

    let status = Command::new("cargo")
        .args(&[
            "install",
            "naclac",
            "--version",
            &resolved_version,
            "--root",
            &root_str,
        ])
        .status();

    let exe_name = format!("naclac{}", env::consts::EXE_SUFFIX);
    let expected_bin_path = root_path.join("bin").join(&exe_name);

    match status {
        Ok(s) if s.success() && expected_bin_path.exists() => {
            println!("{} installed successfully at {:?}", "Version".green().bold(), root_path);
        }
        _ => {
            println!("{} Installation failed or was interrupted. Cleaning up...", "Error:".red().bold());
            let _ = fs::remove_dir_all(&root_path);
        }
    }
}
