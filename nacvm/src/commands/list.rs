use crate::config::Config;
use crate::utils::active;
use colored::*;
use std::fs;

pub fn execute(config: &Config) {
    let active_version = active::get_active_version(config);

    if let Ok(entries) = fs::read_dir(&config.versions_dir) {
        let mut versions = Vec::new();

        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let bin_name = format!("naclac{}", std::env::consts::EXE_SUFFIX);
                let bin_path = entry.path().join("bin").join(&bin_name);

                if bin_path.exists() {
                    if let Some(folder_name) = entry.file_name().to_str() {
                        versions.push(folder_name.to_string());
                    }
                }
            }
        }

        if versions.is_empty() {
            println!("No versions installed.");
            return;
        }

        versions.sort();

        for version in versions {
            if Some(&version) == active_version.as_ref() {
                println!("* {} {}", version.green().bold(), "(active)".green());
            } else {
                println!("  {}", version);
            }
        }
    } else {
        println!("No versions installed.");
    }
}
