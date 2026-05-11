use std::process::Command;

pub fn get_latest_version() -> Option<String> {
    let search_output = Command::new("cargo")
        .args(&["search", "naclac", "--limit", "1"])
        .output()
        .ok()?;

    let output_str = String::from_utf8_lossy(&search_output.stdout);
    if let Some(line) = output_str.lines().find(|l| l.starts_with("naclac = ")) {
        if let Some(v) = line.split('"').nth(1) {
            return Some(v.to_string());
        }
    }
    None
}
