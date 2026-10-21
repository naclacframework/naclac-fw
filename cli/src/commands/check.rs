use std::{fs, path::PathBuf, process::Command};

/// Runs `cargo clippy` against each program in the workspace, one at a time,
/// mirroring `build.rs`'s program discovery. Each program is checked with its
/// own default features, so the runtime and layout it declares are the ones
/// linted.
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

    let every_program_dir: Vec<PathBuf> = fs::read_dir(&programs_dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && path.join("src/lib.rs").exists())
                .collect()
        })
        .unwrap_or_default();
    crate::commands::generate::ensure_client_stubs(&workspace_root, &every_program_dir);

    let mut overall_success = true;

    for program_dir in &program_dirs {
        let prog_name = program_dir.file_name().unwrap().to_str().unwrap();
        let cargo_toml_path = program_dir.join("Cargo.toml");

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

        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
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
