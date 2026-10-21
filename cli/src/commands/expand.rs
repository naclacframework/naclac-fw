use std::{fs, path::PathBuf, process::Command};

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
        eprintln!("❌ Error: No matching program found to expand.");
        std::process::exit(1);
    }

    // Expand the first matching program (usually only one)
    let program_dir = &program_dirs[0];
    let program_name = program_dir.file_name().unwrap().to_str().unwrap();
    let cargo_toml_path = program_dir.join("Cargo.toml");

    eprintln!("⚡ Expanding macros for '{}'...", program_name);

    let mut cmd = Command::new("cargo");
    cmd.arg("expand")
        .arg("--manifest-path")
        .arg(cargo_toml_path.to_str().unwrap())
        .current_dir(&workspace_root);

    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
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
