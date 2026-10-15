use std::path::{Path, PathBuf};
use std::process::Command;

pub fn read_target_dir_from_cargo_config(workspace_root: &Path) -> Option<PathBuf> {
    for config_name in &["config.toml", "config"] {
        let config_path = workspace_root.join(".cargo").join(config_name);
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            if let Ok(parsed) = toml::from_str::<toml::Value>(&contents) {
                if let Some(target_dir) = parsed
                    .get("build")
                    .and_then(|b| b.get("target-dir"))
                    .and_then(|v| v.as_str())
                {
                    let path = PathBuf::from(target_dir);
                    return Some(if path.is_absolute() {
                        path
                    } else {
                        workspace_root.join(path)
                    });
                }
            }
        }
    }
    None
}

fn read_target_dir_from_env(workspace_root: &Path) -> Option<PathBuf> {
    std::env::var("CARGO_TARGET_DIR").ok().and_then(|val| {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            None
        } else {
            let path = PathBuf::from(trimmed);
            Some(if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            })
        }
    })
}

/// Resolves the cargo target directory for `workspace_root`, honoring
/// `CARGO_TARGET_DIR` and `.cargo/config.toml`'s `target-dir`.
pub fn resolve_target_dir(workspace_root: &Path) -> PathBuf {
    let manifest_path = workspace_root.join("Cargo.toml");
    if manifest_path.exists() {
        if let Ok(output) = Command::new("cargo")
            .arg("metadata")
            .arg("--no-deps")
            .arg("--format-version")
            .arg("1")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .output()
        {
            if output.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(dir) = json.get("target_directory").and_then(|v| v.as_str()) {
                        return PathBuf::from(dir);
                    }
                }
            }
        }
    }
    read_target_dir_from_env(workspace_root)
        .or_else(|| read_target_dir_from_cargo_config(workspace_root))
        .unwrap_or_else(|| workspace_root.join("target"))
}

pub fn find_naclac_framework_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.canonicalize().ok()?;
    for _ in 0..7 {
        if dir.join("naclac-lang/Cargo.toml").exists()
            && dir.join("naclac-client/Cargo.toml").exists()
        {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
    None
}

pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from_comps: Vec<_> = from.components().collect();
    let to_comps: Vec<_> = to.components().collect();
    let common_len = from_comps
        .iter()
        .zip(to_comps.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut result = PathBuf::new();
    for _ in common_len..from_comps.len() {
        result.push("..");
    }
    for comp in &to_comps[common_len..] {
        result.push(comp.as_os_str());
    }
    result
}

pub fn naclac_dep_paths(workspace_root: &Path, clients_dir: &Path) -> Option<(String, String)> {
    let root = find_naclac_framework_root(workspace_root)?;
    let canon_ws = workspace_root.canonicalize().ok()?;
    let canon_clients = if let Ok(canon) = clients_dir.canonicalize() {
        canon
    } else if clients_dir.is_relative() {
        canon_ws.join(clients_dir)
    } else {
        clients_dir.to_path_buf()
    };
    let lang_path = relative_path(&canon_clients, &root.join("naclac-lang"));
    let client_path = relative_path(&canon_clients, &root.join("naclac-client"));
    Some((
        lang_path.to_string_lossy().replace('\\', "/"),
        client_path.to_string_lossy().replace('\\', "/"),
    ))
}

pub fn naclac_dep_path_fragments(workspace_root: &Path, clients_dir: &Path) -> (String, String) {
    match naclac_dep_paths(workspace_root, clients_dir) {
        Some((lang, client)) => (
            format!(", path = \"{lang}\""),
            format!(", path = \"{client}\""),
        ),
        None => (String::new(), String::new()),
    }
}

pub fn naclac_ts_dep_specs(workspace_root: &Path) -> (String, String) {
    if let Some(root) = find_naclac_framework_root(workspace_root) {
        if let Ok(canon_ws) = workspace_root.canonicalize() {
            let client_target = root.join("client");
            let litesvm_target = root.join("naclac-litesvm-node");
            if client_target.join("package.json").exists()
                && litesvm_target.join("package.json").exists()
            {
                let client_rel = relative_path(&canon_ws, &client_target)
                    .to_string_lossy()
                    .replace('\\', "/");
                let litesvm_rel = relative_path(&canon_ws, &litesvm_target)
                    .to_string_lossy()
                    .replace('\\', "/");
                return (format!("link:{client_rel}"), format!("link:{litesvm_rel}"));
            }
        }
    }
    ("0.1.0".to_string(), "0.1.0".to_string())
}
