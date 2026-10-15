use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Runs `cargo clippy` against each program in the workspace, one at a time —
/// mirroring `build.rs`'s program discovery and feature detection (so a
/// pinocchio program is checked with the exact same `--no-default-features
/// --features pinocchio` flags it's actually built with), rather than asking
/// users to remember and re-type each program's correct feature combination
/// by hand every time.
pub fn execute(program_id: Option<&str>, features: Vec<String>) {
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

        eprintln!("🔍 Checking '{}' with clippy...", prog_name);

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--manifest-path")
            .arg(cargo_toml_path.to_str().unwrap())
            .current_dir(&workspace_root);

        let mut all_features = features.clone();
        if use_pinocchio {
            all_features.push("pinocchio".to_string());
            cmd.arg("--no-default-features");
        }
        if !all_features.is_empty() {
            cmd.arg("--features").arg(all_features.join(","));
        }

        let mut child = cmd
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute cargo clippy. Make sure clippy is installed: rustup component add clippy");

        let status = child.wait().expect("Failed to wait on cargo clippy");

        if !status.success() {
            eprintln!("❌ Clippy check failed for '{}'.", prog_name);
            overall_success = false;
            break;
        }
    }

    if overall_success {
        eprintln!("✅ All programs passed clippy checks!");
    } else {
        std::process::exit(1);
    }
}
