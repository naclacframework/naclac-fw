use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
                let is_target = program_id.map_or(true, |tgt| path.file_name().unwrap() == tgt);
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
        eprintln!("❌ Error: No matching program found to expand.");
        std::process::exit(1);
    }

    // Expand the first matching program (usually only one)
    let program_dir = &program_dirs[0];
    let program_name = program_dir.file_name().unwrap().to_str().unwrap();
    let cargo_toml_path = program_dir.join("Cargo.toml");

    eprintln!("🔍 Detecting features for '{}'...", program_name);

    // Detect active features from Cargo.toml
    let mut active_features = Vec::new();
    if cargo_toml_path.exists() {
        let content = fs::read_to_string(&cargo_toml_path).unwrap_or_default();
        let mut in_features = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[features]") {
                in_features = true;
                continue;
            }
            if trimmed.starts_with("[") {
                in_features = false;
            }
            
            if in_features {
                if trimmed.starts_with("idl-build") {
                    active_features.push("idl-build");
                }
            }

            if trimmed.starts_with("pinocchio") || (trimmed.starts_with("default") && trimmed.contains("pinocchio")) {
                active_features.push("pinocchio");
            }
        }
    }

    eprintln!("⚡ Expanding macros with features: {:?}...", active_features);

    let mut cmd = Command::new("cargo");
    cmd.arg("expand")
        .arg("--manifest-path")
        .arg(cargo_toml_path.to_str().unwrap())
        .current_dir(&workspace_root);

    let mut final_features = active_features.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    for f in features {
        if !final_features.contains(&f) {
            final_features.push(f);
        }
    }

    if !final_features.is_empty() {
        cmd.arg("--features").arg(final_features.join(","));
        if final_features.contains(&"pinocchio".to_string()) {
            cmd.arg("--no-default-features");
        }
    }

    let mut child = cmd
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("Failed to execute cargo expand. Make sure cargo-expand is installed: cargo install cargo-expand");

    let status = child.wait().expect("Failed to wait on cargo expand");

    if !status.success() {
        eprintln!("❌ Cargo expand failed for '{}'.", program_name);
        std::process::exit(1);
    }
}
