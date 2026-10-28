use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// The version of the published naclac packages a workspace without a
/// `[framework]` section depends on.
const PUBLISHED_VERSION: &str = "0.1.0";

/// The cargo target directory of `workspace_root`, as `cargo metadata` reports
/// it (so `CARGO_TARGET_DIR` and `.cargo/config.toml`'s `target-dir` apply).
/// Fails when the workspace has no `Cargo.toml` or cargo cannot read it.
pub fn resolve_target_dir(workspace_root: &Path) -> Result<PathBuf, String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .output()
        .map_err(|e| format!("could not run `cargo metadata`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed for {}: {}",
            manifest_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("`cargo metadata` printed invalid JSON: {e}"))?;
    metadata
        .get("target_directory")
        .and_then(|dir| dir.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "`cargo metadata` did not report a `target_directory`".to_string())
}

pub const ARTIFACTS_DIR_NAME: &str = "artifacts";

/// Root of everything naclac generates or must keep for `workspace_root`;
/// independent of cargo's target directory.
pub fn artifacts_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(ARTIFACTS_DIR_NAME)
}

/// Holds `<name>.json` and `<name>.ts` for every program.
pub fn idl_dir(workspace_root: &Path) -> PathBuf {
    artifacts_dir(workspace_root).join("idl")
}

/// Holds the generated `rust/` and `typescript/` clients.
pub fn clients_dir(workspace_root: &Path) -> PathBuf {
    artifacts_dir(workspace_root).join("clients")
}

/// Holds each program's `<name>.so` next to its `<name>-keypair.json`. The two
/// must stay co-located: `cargo build-sbf --sbf-out-dir`, `solana program
/// deploy`, and `naclac-client`'s litesvm loader all derive the program ID
/// from the keypair beside the `.so`.
pub fn deploy_dir(workspace_root: &Path) -> PathBuf {
    artifacts_dir(workspace_root).join("deploy")
}

/// The local framework checkout the workspace builds against: `[framework]
/// path` in the workspace's Naclac.toml, relative to the workspace root.
/// `None` when there is no `[framework]` section, in which case the workspace
/// depends on the published packages. Fails when Naclac.toml cannot be read
/// or parsed, when `[framework]` has no string `path`, or when that folder is
/// not a framework checkout.
pub fn framework_root(workspace_root: &Path) -> Result<Option<PathBuf>, String> {
    let config_path = workspace_root.join("Naclac.toml");
    let text = fs::read_to_string(&config_path)
        .map_err(|e| format!("cannot read {}: {e}", config_path.display()))?;
    let config: toml::Value = toml::from_str(&text)
        .map_err(|e| format!("{} is not valid TOML: {e}", config_path.display()))?;
    let Some(section) = config.get("framework") else {
        return Ok(None);
    };
    let toml::Value::Table(section) = section else {
        return Err(format!(
            "`[framework]` in {} must be a table",
            config_path.display()
        ));
    };
    let path = match section.get("path") {
        Some(toml::Value::String(path)) => path,
        Some(_) => {
            return Err(format!(
                "`path` under `[framework]` in {} must be a string",
                config_path.display()
            ))
        }
        None => {
            return Err(format!(
                "`[framework]` in {} needs a `path` to the framework checkout",
                config_path.display()
            ))
        }
    };
    let candidate = workspace_root.join(path);
    let root = fs::canonicalize(&candidate).map_err(|e| {
        format!(
            "`[framework] path = \"{path}\"` in {}: cannot read {}: {e}",
            config_path.display(),
            candidate.display()
        )
    })?;
    for required in ["lang/Cargo.toml", "client-rs/Cargo.toml"] {
        let file = root.join(required);
        if !file.is_file() {
            return Err(format!(
                "`[framework] path = \"{path}\"` in {} is not a naclac framework checkout: {} \
                 is missing",
                config_path.display(),
                file.display()
            ));
        }
    }
    Ok(Some(root))
}

/// How many folders above a new project `naclac init` looks for a framework
/// checkout.
const MAX_FRAMEWORK_SEARCH_DEPTH: usize = 7;

/// For `naclac init` only: the framework checkout `start_dir` sits inside, if
/// any (the nearest ancestor holding `lang/Cargo.toml` and
/// `client-rs/Cargo.toml`). The result is written into the new workspace's
/// Naclac.toml as `[framework] path`; every other command reads that
/// recorded value and never searches.
pub fn find_framework_checkout(start_dir: &Path) -> Result<Option<PathBuf>, String> {
    let mut dir = fs::canonicalize(start_dir)
        .map_err(|e| format!("cannot read {}: {e}", start_dir.display()))?;
    for _ in 0..=MAX_FRAMEWORK_SEARCH_DEPTH {
        if dir.join("lang/Cargo.toml").is_file() && dir.join("client-rs/Cargo.toml").is_file() {
            return Ok(Some(dir));
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(None)
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

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `target` relative to `dir`, a folder inside `workspace_root`.
fn relative_to_workspace_dir(
    workspace_root: &Path,
    dir: &Path,
    target: &Path,
) -> Result<PathBuf, String> {
    let inside = dir.strip_prefix(workspace_root).map_err(|_| {
        format!(
            "{} is not inside the workspace {}",
            dir.display(),
            workspace_root.display()
        )
    })?;
    let canonical_workspace = fs::canonicalize(workspace_root)
        .map_err(|e| format!("cannot read {}: {e}", workspace_root.display()))?;
    Ok(relative_path(&canonical_workspace.join(inside), target))
}

/// The `path = "..."` fragments to append to the `naclac-lang` and
/// `naclac-client` dependency lines of a manifest in `manifest_dir` (a folder
/// inside `workspace_root`): empty without a `[framework]` section, so the
/// published packages are used.
pub fn naclac_dep_path_fragments(
    workspace_root: &Path,
    manifest_dir: &Path,
) -> Result<(String, String), String> {
    let Some(root) = framework_root(workspace_root)? else {
        return Ok((String::new(), String::new()));
    };
    let fragment = |package: &str| -> Result<String, String> {
        let relative =
            relative_to_workspace_dir(workspace_root, manifest_dir, &root.join(package))?;
        Ok(format!(", path = \"{}\"", slashed(&relative)))
    };
    Ok((fragment("lang")?, fragment("client-rs")?))
}

/// The `@naclac-fw/client` and `@naclac-fw/litesvm` dependency specs of the
/// workspace's `package.json`: `link:` paths into the `[framework]` checkout,
/// or the published version without a `[framework]` section.
pub fn naclac_ts_dep_specs(workspace_root: &Path) -> Result<(String, String), String> {
    let Some(root) = framework_root(workspace_root)? else {
        return Ok((PUBLISHED_VERSION.to_string(), PUBLISHED_VERSION.to_string()));
    };
    let canonical_workspace = fs::canonicalize(workspace_root)
        .map_err(|e| format!("cannot read {}: {e}", workspace_root.display()))?;
    let spec = |package: &str| -> Result<String, String> {
        let target = root.join(package);
        if !target.join("package.json").is_file() {
            return Err(format!(
                "the framework checkout at {} has no {package}/package.json",
                root.display()
            ));
        }
        Ok(format!(
            "link:{}",
            slashed(&relative_path(&canonical_workspace, &target))
        ))
    };
    Ok((spec("client")?, spec("litesvm-node")?))
}
