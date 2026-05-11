use crate::config::Config;
use crate::utils::router;
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

    let root_path = config.versions_dir.join(&resolved_version);
    
    let bin_name = format!("naclac{}", env::consts::EXE_SUFFIX);
    let bin_path = root_path.join("bin").join(&bin_name);

    if !bin_path.exists() {
        println!("{} Version {} is not installed. Run `nacvm install {}` first.", "Error:".red().bold(), resolved_version, version);
        return;
    }

    match router::create_router(config, &resolved_version) {
        Ok(_) => {
            println!("{} Using version {}", "Success:".green().bold(), resolved_version);
        }
        Err(e) => {
            println!("{} Failed to update active version: {}", "Error:".red().bold(), e);
        }
    }
}
