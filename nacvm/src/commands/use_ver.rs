use crate::config::Config;
use crate::utils::router;
use std::env;
use std::fs;
use colored::*;

fn get_active_version(config: &Config) -> Option<String> {
    let os = std::env::consts::OS;
    if os == "windows" {
        let cmd_path = config.bin_dir.join("naclac.cmd");
        if cmd_path.exists() {
            if let Ok(contents) = fs::read_to_string(&cmd_path) {
                let versions_dir_str = config.versions_dir.to_string_lossy().to_string();
                if let Some(start_idx) = contents.find(&versions_dir_str) {
                    let sub = &contents[start_idx + versions_dir_str.len()..];
                    let parts: Vec<&str> = sub.split(|c| c == '\\' || c == '/').filter(|s| !s.is_empty()).collect();
                    if !parts.is_empty() {
                        return Some(parts[0].to_string());
                    }
                }
            }
        }
    } else {
        let router_path = config.bin_dir.join("naclac");
        if let Ok(target) = fs::read_link(&router_path) {
            let target_str = target.to_string_lossy();
            let versions_dir_str = config.versions_dir.to_string_lossy().to_string();
            if let Some(start_idx) = target_str.find(&versions_dir_str) {
                let sub = &target_str[start_idx + versions_dir_str.len()..];
                let parts: Vec<&str> = sub.split(|c| c == '/' || c == '\\').filter(|s| !s.is_empty()).collect();
                if !parts.is_empty() {
                    return Some(parts[0].to_string());
                }
            }
        }
    }
    None
}

pub fn execute(config: &Config, version: &str) {
    let mut resolved_version = version.to_string();

    if version.to_lowercase() == "latest" {
        println!("🔍 Querying latest stable release from GitHub...");
        if let Some(v) = crate::utils::resolve::get_latest_stable_version() {
            resolved_version = v;
            println!("🔍 Found latest stable version: v{}", resolved_version);
        } else {
            eprintln!(
                "{} No stable version found. Specify a version if you want to use a pre-release version.",
                "Error:".red().bold()
            );
            std::process::exit(1);
        }
    }

    // Strip leading 'v' if present for local directory matching
    if resolved_version.starts_with('v') {
        resolved_version = resolved_version[1..].to_string();
    }

    let root_path = config.versions_dir.join(&resolved_version);
    let bin_name = format!("naclac{}", env::consts::EXE_SUFFIX);
    let bin_path = root_path.join("bin").join(&bin_name);

    if !bin_path.exists() {
        println!("{} Version {} is not installed. Run `nacvm install {}` first.", "Error:".red().bold(), resolved_version, version);
        return;
    }

    // Check if the requested version is already active
    if let Some(active_version) = get_active_version(config) {
        if active_version == resolved_version {
            println!("{} Version {} is already in use.", "Info:".blue().bold(), resolved_version);
            return;
        }
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
