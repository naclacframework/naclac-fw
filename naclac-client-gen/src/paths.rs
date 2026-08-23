use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolves the real cargo target directory for `workspace_root`, honoring
/// `CARGO_TARGET_DIR` and `.cargo/config.toml`'s `target-dir` the same way
/// `cargo` itself does — by asking `cargo metadata` directly rather than
/// re-implementing cargo's own env-var/config-file resolution order. Falls
/// back to `workspace_root/target` if `cargo metadata` can't be run at all
/// (e.g. no `Cargo.toml` present yet, cargo missing from `PATH`).
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
    workspace_root.join("target")
}

pub fn find_naclac_framework_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.canonicalize().ok()?;
    loop {
        if dir.join("naclac-lang/Cargo.toml").exists()
            && dir.join("naclac-client/Cargo.toml").exists()
        {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
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

/// Relative path (using forward slashes, for Cargo.toml portability) from a
/// generated client's own directory to `naclac-lang`/`naclac-client`,
/// discovered dynamically rather than assumed at a fixed nesting depth.
/// `workspace_root` must already exist (used to search upward for the
/// naclac-fw root); `clients_dir` need not — it only needs to already be
/// absolute and normalized (i.e. derived from `workspace_root` by joining
/// path components, not containing its own `..`/symlink segments).
pub fn naclac_dep_paths(workspace_root: &Path, clients_dir: &Path) -> Option<(String, String)> {
    let root = find_naclac_framework_root(workspace_root)?;
    let lang_path = relative_path(clients_dir, &root.join("naclac-lang"));
    let client_path = relative_path(clients_dir, &root.join("naclac-client"));
    Some((
        lang_path.to_string_lossy().replace('\\', "/"),
        client_path.to_string_lossy().replace('\\', "/"),
    ))
}

/// `(naclac-lang path fragment, naclac-client path fragment)`, each either
/// `, path = "..."` (ready to splice directly after the crate name in a
/// Cargo.toml dependency line) or an empty string when the naclac-fw root
/// couldn't be located — in which case a warning is printed and the
/// resulting Cargo.toml is deliberately left with no `path`, so `cargo`'s
/// own dependency resolution produces the real error when it actually runs,
/// rather than this tool guessing a path that may or may not be correct.
pub fn naclac_dep_path_fragments(workspace_root: &Path, clients_dir: &Path) -> (String, String) {
    match naclac_dep_paths(workspace_root, clients_dir) {
        Some((lang, client)) => (
            format!(", path = \"{lang}\""),
            format!(", path = \"{client}\""),
        ),
        None => {
            eprintln!(
                "⚠️  Could not locate 'naclac-lang'/'naclac-client' on disk relative to {:?} — \
                 generating their Cargo.toml dependency entries without a `path`. This will \
                 likely fail to resolve when the build actually runs.",
                workspace_root
            );
            (String::new(), String::new())
        }
    }
}
