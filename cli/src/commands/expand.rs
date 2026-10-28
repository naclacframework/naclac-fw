use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    std::process::Command,
};

/// Expands the macros of one program. `requested` names it; it may be empty
/// only when Naclac.toml lists exactly one program, because an expansion is
/// large and one is always meant.
pub fn execute(requested: &[String], features: Vec<String>) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let listed = programs::listed(&config);
    let selected = programs::select(&listed, requested);
    let [program_name] = selected.as_slice() else {
        ui::error(format!(
            "`naclac expand` works on one program at a time; choose one with `-p <program>`. \
             Available: {}.",
            listed.join(", ")
        ));
    };

    let program_dir = programs::program_dir(&workspace_root, program_name);
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
