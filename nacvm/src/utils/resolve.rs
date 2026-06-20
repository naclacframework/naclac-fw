use std::process::Command;
use serde::Deserialize;

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
}

fn fetch_github_release(url: &str) -> Option<GitHubRelease> {
    let os = std::env::consts::OS;
    let output = if os == "windows" {
        Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                &format!("$ProgressPreference = 'SilentlyContinue'; Invoke-RestMethod -Uri '{}' | ConvertTo-Json", url),
            ])
            .output()
            .ok()?
    } else {
        Command::new("curl")
            .args(&["-sSfL", url])
            .output()
            .ok()?
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).ok()
}

pub fn get_latest_stable_version() -> Option<String> {
    let url = "https://api.github.com/repos/naclacframework/naclac-fw/releases/latest";
    let release = fetch_github_release(url)?;
    // Strip leading 'v' if present for folder naming and download compatibility
    let version = if release.tag_name.starts_with('v') {
        release.tag_name[1..].to_string()
    } else {
        release.tag_name
    };
    Some(version)
}

pub fn get_specific_version_info(version: &str) -> Option<(String, bool)> {
    let check_version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{}", version)
    };
    let url = format!("https://api.github.com/repos/naclacframework/naclac-fw/releases/tags/{}", check_version);
    let release = fetch_github_release(&url)?;
    let clean_version = if release.tag_name.starts_with('v') {
        release.tag_name[1..].to_string()
    } else {
        release.tag_name
    };
    Some((clean_version, release.prerelease))
}
