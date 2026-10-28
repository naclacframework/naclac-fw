//! `naclac clean` — removes the build output of the selected programs (every
//! listed program unless `-p` narrows it), in both the dev and release
//! profiles: the host build with `cargo clean -p`, and the on-chain build of
//! every `<triple>-solana-solana` folder in the target directory with
//! `cargo clean -p --target`, using the platform-tools `rustc` because the
//! host `rustc` does not know the Solana triples. Dependencies stay cached.
//! `--hard` instead runs a single `cargo clean`, which removes cargo's whole
//! target directory wherever `CARGO_TARGET_DIR` or `.cargo/config.toml` puts
//! it. `artifacts/` (the program keypairs and everything generated) lives
//! outside the target directory and is never touched; `--hard` refuses to run
//! if the target directory would contain it.

use {
    crate::{
        commands::{build, generate, programs, workspace},
        ui,
    },
    std::{fs, path::Path, process::Command},
};

const SOLANA_TRIPLE_SUFFIX: &str = "-solana-solana";

pub fn execute(requested: &[String], hard: bool) {
    let workspace_root = workspace::root();
    let target_dir =
        naclac_client_gen::resolve_target_dir(&workspace_root).unwrap_or_else(|e| ui::error(e));

    if hard {
        let artifacts_dir = naclac_client_gen::artifacts_dir(&workspace_root);
        if artifacts_dir.starts_with(&target_dir) {
            ui::error(format!(
                "Refusing to clean: the cargo target directory {} contains {}, which holds the \
                 program keypairs. Move the target directory out of it.",
                target_dir.display(),
                artifacts_dir.display()
            ));
        }
        println!("🧹 Running cargo clean...");
        run(Command::new("cargo")
            .arg("clean")
            .current_dir(&workspace_root));
    } else {
        let config = workspace::config(&workspace_root);
        let selected = programs::select(&programs::listed(&config), requested);
        let packages: Vec<String> = selected
            .iter()
            .map(|name| generate::read_package_name(&programs::program_dir(&workspace_root, name)))
            .collect();
        println!("🧹 Cleaning {}...", selected.join(", "));

        for release in [false, true] {
            let mut host = Command::new("cargo");
            host.arg("clean").current_dir(&workspace_root);
            if release {
                host.arg("--release");
            }
            for package in &packages {
                host.arg("-p").arg(package);
            }
            run(&mut host);
        }

        let triples = solana_triples(&target_dir);
        if !triples.is_empty() {
            let (tools_version, _) = build::resolve_build_toolchain(&config);
            let rustc = workspace::home_dir()
                .join(".cache/solana")
                .join(&tools_version)
                .join("platform-tools/rust/bin/rustc");
            let installed = rustc.try_exists().unwrap_or_else(|e| {
                ui::error(format!("Cannot check for {}: {e}", rustc.display()))
            });
            if !installed {
                ui::error(format!(
                    "Cannot clean the on-chain build: platform-tools {tools_version} is not \
                     installed ({} is missing). Run `naclac build` once to install it.",
                    rustc.display()
                ));
            }
            for triple in &triples {
                for release in [false, true] {
                    let mut sbf = Command::new("cargo");
                    sbf.arg("clean")
                        .arg("--target")
                        .arg(triple)
                        .env("RUSTC", &rustc)
                        .current_dir(&workspace_root);
                    if release {
                        sbf.arg("--release");
                    }
                    for package in &packages {
                        sbf.arg("-p").arg(package);
                    }
                    run(&mut sbf);
                }
            }
        }
    }
    ui::success("Build output removed. Program keypairs in artifacts/ are never touched.");
}

fn solana_triples(target_dir: &Path) -> Vec<String> {
    let exists = target_dir
        .try_exists()
        .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", target_dir.display())));
    if !exists {
        return Vec::new();
    }
    let entries = fs::read_dir(target_dir)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", target_dir.display())));
    let mut triples = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| {
            ui::error(format!(
                "Cannot read an entry of {}: {e}",
                target_dir.display()
            ))
        });
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(SOLANA_TRIPLE_SUFFIX) {
            triples.push(name);
        }
    }
    triples.sort();
    triples
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|e| ui::error(format!("Failed to run cargo clean: {e}")));
    if !status.success() {
        ui::error("cargo clean failed.");
    }
}
