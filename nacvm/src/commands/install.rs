use crate::config::Config;
use std::process::{Command, Stdio};
use std::fs;
use std::env;
use colored::*;
use std::thread;
use std::time::Duration;
use std::io::{self, Write};

pub fn execute(config: &Config, version: &str) {
    let resolved_version = if version.to_lowercase() == "latest" {
        println!("🔍 Querying latest stable release from GitHub...");
        if let Some(v) = crate::utils::resolve::get_latest_stable_version() {
            println!("🔍 Found latest stable version: v{}", v);
            v
        } else {
            eprintln!(
                "{} No stable version found. Specify a version if you have to download a pre-release version.",
                "Error:".red().bold()
            );
            std::process::exit(1);
        }
    } else {
        let mut clean_ver = version.to_string();
        if clean_ver.starts_with('v') {
            clean_ver = clean_ver[1..].to_string();
        }
        let root_path = config.versions_dir.join(&clean_ver);
        let exe_name = format!("naclac{}", env::consts::EXE_SUFFIX);
        if root_path.join("bin").join(&exe_name).exists() {
            println!("{} Version {} is already installed.", "Info:".blue().bold(), clean_ver);
            return;
        }

        println!("🔍 Querying version information from GitHub...");
        if let Some((v, is_prerelease)) = crate::utils::resolve::get_specific_version_info(version) {
            if is_prerelease {
                println!(
                    "{} Downloading the pre-release version of v{}",
                    "Warning:".yellow().bold(),
                    v
                );
            } else {
                println!("🔍 Downloading stable version v{}...", v);
            }
            v
        } else {
            eprintln!(
                "{} Version {} not found.",
                "Error:".red().bold(),
                version
            );
            std::process::exit(1);
        }
    };


    let root_path = config.versions_dir.join(&resolved_version);
    let exe_name = format!("naclac{}", env::consts::EXE_SUFFIX);
    let expected_bin_path = root_path.join("bin").join(&exe_name);

    if expected_bin_path.exists() {
        println!("{} Version {} is already installed.", "Info:".blue().bold(), resolved_version);
        return;
    }

    println!("{} naclac v{}...", "Installing".green().bold(), resolved_version);

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
    
    if !download_with_progress(&download_url, &temp_archive) {
        eprintln!("{} Failed to download pre-compiled binary.", "Error:".red().bold());
        let _ = fs::remove_dir_all(&root_path);
        let _ = fs::remove_file(&temp_archive);
        std::process::exit(1);
    }

    println!("🚚 Extracting binary...");

    let extract_status = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-NoProfile",
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

fn get_content_length(url: &str) -> Option<u64> {
    let os = std::env::consts::OS;
    let output = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                &format!(
                    "$ProgressPreference = 'SilentlyContinue'; $req = [System.Net.WebRequest]::Create('{}'); $req.Method = 'HEAD'; try {{ $res = $req.GetResponse(); $res.ContentLength; $res.Close() }} catch {{ 0 }}",
                    url
                ),
            ])
            .output()
            .ok()?
    } else {
        Command::new("curl")
            .args(&["-sIL", url])
            .output()
            .ok()?
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if os == "windows" {
        stdout.trim().parse::<u64>().ok()
    } else {
        let mut length = None;
        for line in stdout.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    if let Ok(num) = val.trim().parse::<u64>() {
                        length = Some(num);
                    }
                }
            }
        }
        length
    }
}

fn download_with_progress(url: &str, dest: &std::path::Path) -> bool {
    let os = std::env::consts::OS;
    let total_bytes = get_content_length(url).unwrap_or(0);

    if dest.exists() {
        let _ = fs::remove_file(dest);
    }

    let child = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                &format!(
                    "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                    url,
                    dest.to_string_lossy()
                ),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    } else {
        Command::new("curl")
            .args(&[
                "-sSfL",
                "-o",
                &dest.to_string_lossy(),
                url,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    };

    if child.is_none() {
        return false;
    }
    let mut child = child.unwrap();

    let spin = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut spin_idx = 0;
    let bar_width = 30;

    loop {
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                if total_bytes > 0 {
                    let final_bar = "=".repeat(bar_width);
                    let mb_total = total_bytes as f64 / 1048576.0;
                    print!(
                        "\r{} {} [ {} ] 100% ({:.2} MiB / {:.2} MiB)   \n",
                        "✅".green(),
                        "Downloading".green().bold(),
                        final_bar,
                        mb_total,
                        mb_total
                    );
                } else {
                    println!("\r{} Download complete!", "✅".green());
                }
                io::stdout().flush().unwrap();
                return true;
            } else {
                return false;
            }
        }

        let current_bytes = if dest.exists() {
            fs::metadata(dest).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let spin_char = spin[spin_idx % 10];
        spin_idx += 1;

        if total_bytes > 0 {
            let pct = (current_bytes as f64 / total_bytes as f64 * 100.0) as usize;
            let filled = (pct * bar_width / 100) as usize;
            let empty = bar_width - filled;

            let mut bar_str = "=".repeat(filled);
            if empty > 0 {
                bar_str.push('>');
                if empty > 1 {
                    bar_str.push_str(&" ".repeat(empty - 1));
                }
            }

            let mb_read = current_bytes as f64 / 1048576.0;
            let mb_total = total_bytes as f64 / 1048576.0;

            print!(
                "\r{} 🚚 Downloading [ {} ] {}% ({:.2} MiB / {:.2} MiB)   ",
                spin_char,
                bar_str,
                pct,
                mb_read,
                mb_total
            );
        } else {
            let mb_read = current_bytes as f64 / 1048576.0;
            print!("\r{} 🚚 Downloading ({:.2} MiB)...", spin_char, mb_read);
        }
        io::stdout().flush().unwrap();

        thread::sleep(Duration::from_millis(100));
    }
}
