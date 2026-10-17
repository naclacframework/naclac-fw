use std::{fs, path::PathBuf, process::Command};

/// Runs `cargo clippy` against each program in the workspace, one at a time —
/// mirroring `build.rs`'s program discovery and feature detection (so a
/// pinocchio program is checked with the exact same `--no-default-features
/// --features pinocchio` flags it's actually built with), rather than asking
/// users to remember and re-type each program's correct feature combination
/// by hand every time.
///
/// With `tests` set, each program's test targets (its unit tests and the
/// integration tests under `tests/`) are linted instead of the program itself.
/// Those need the program's generated client, so run `naclac build` first.
///
/// With `deny_warnings` set, any clippy warning fails the check like an error.
pub fn execute(program_id: Option<&str>, features: Vec<String>, tests: bool, deny_warnings: bool) {
    let current_dir = std::env::current_dir().unwrap();

    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        eprintln!(
            "❌ Error: Could not find Naclac.toml. Please run from within a Naclac workspace."
        );
        std::process::exit(1);
    };

    let programs_dir = workspace_root.join("programs");
    let program_dirs: Vec<PathBuf> = if programs_dir.exists() {
        fs::read_dir(&programs_dir)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let is_target = program_id.is_none_or(|tgt| path.file_name().unwrap() == tgt);
                if path.is_dir() && path.join("src/lib.rs").exists() && is_target {
                    Some(path)
                } else {
                    None
                }
            })
            .collect()
    } else {
        eprintln!("❌ Error: No valid Naclac programs found in workspace.");
        std::process::exit(1);
    };

    if program_dirs.is_empty() {
        eprintln!("❌ Error: No matching program found to check.");
        std::process::exit(1);
    }

    let mut overall_success = true;

    for program_dir in &program_dirs {
        let prog_name = program_dir.file_name().unwrap().to_str().unwrap();
        let cargo_toml_path = program_dir.join("Cargo.toml");

        // Same detection as build.rs: read the program's own Cargo.toml to
        // decide whether it's a pinocchio program (--no-default-features
        // --features pinocchio) or a standard one, rather than guessing.
        let mut use_pinocchio = false;
        if cargo_toml_path.exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                    if let Some(feats) = parsed.get("features").and_then(|f| f.as_table()) {
                        if let Some(default_feats) = feats.get("default").and_then(|d| d.as_array())
                        {
                            use_pinocchio = default_feats
                                .iter()
                                .any(|v| v.as_str() == Some("pinocchio"));
                        }
                        if !use_pinocchio {
                            use_pinocchio = feats.contains_key("pinocchio");
                        }
                    }
                }
            }
        }

        if tests {
            eprintln!("🔍 Checking the tests of '{}' with clippy...", prog_name);
        } else {
            eprintln!("🔍 Checking '{}' with clippy...", prog_name);
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--manifest-path")
            .arg(cargo_toml_path.to_str().unwrap())
            .current_dir(&workspace_root);
        if tests {
            cmd.arg("--tests");
        }

        let mut all_features = features.clone();
        if use_pinocchio {
            all_features.push("pinocchio".to_string());
            cmd.arg("--no-default-features");
        }
        if !all_features.is_empty() {
            cmd.arg("--features").arg(all_features.join(","));
        }
        if deny_warnings {
            cmd.args(["--", "-D", "warnings"]);
        }

        let mut child = cmd
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute cargo clippy. Make sure clippy is installed: rustup component add clippy");

        let status = child.wait().expect("Failed to wait on cargo clippy");

        if !status.success() {
            if tests {
                eprintln!(
                    "❌ Clippy check failed for the tests of '{}'. If it could not resolve the \
                     generated client, run `naclac build` first.",
                    prog_name
                );
            } else {
                eprintln!("❌ Clippy check failed for '{}'.", prog_name);
            }
            overall_success = false;
            break;
        }
    }

    if overall_success {
        if tests {
            eprintln!("✅ All program tests passed clippy checks!");
        } else {
            eprintln!("✅ All programs passed clippy checks!");
        }
    } else {
        std::process::exit(1);
    }
}
