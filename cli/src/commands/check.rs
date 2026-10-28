use {
    crate::commands::{programs, workspace},
    std::{path::PathBuf, process::Command},
};

/// Runs `cargo clippy` against each program in `requested` (every program
/// Naclac.toml lists when empty; see `programs`), one at a time. Each program
/// is checked with its own default features, so the runtime and layout it
/// declares are the ones linted.
///
/// With `tests` set, each program's test targets (its unit tests and the
/// integration tests under `tests/`) are linted instead of the program itself.
/// Those need the program's generated client, so run `naclac build` first.
///
/// With `deny_warnings` set, any clippy warning fails the check like an error.
pub fn execute(requested: &[String], features: Vec<String>, tests: bool, deny_warnings: bool) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let program_dirs: Vec<PathBuf> = programs::select(&programs::listed(&config), requested)
        .iter()
        .map(|name| programs::program_dir(&workspace_root, name))
        .collect();

    crate::commands::generate::ensure_client_stubs(
        &workspace_root,
        &programs::all_dirs(&workspace_root),
    );

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
