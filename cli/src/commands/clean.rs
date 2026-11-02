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
        cargo_progress,
        commands::{build, generate, programs, workspace},
        ui,
    },
    std::{
        fs,
        path::Path,
        process::{Command, Stdio},
        time::Instant,
    },
};

const SOLANA_TRIPLE_SUFFIX: &str = "-solana-solana";

pub fn execute(requested: &[String], hard: bool) {
    let workspace_root = workspace::root();
    let target_dir =
        naclac_client_gen::resolve_target_dir(&workspace_root).unwrap_or_else(|e| ui::error(e));

    let start = Instant::now();

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

        ui::header("clean", "target directory");
        ui::kv("target", target_dir.display());

        let mut cmd = Command::new("cargo");
        cmd.arg("clean").current_dir(&workspace_root);

        run_clean_command(&mut cmd, "target directory cleaned");
        ui::milestone(format!(
            "Cleaned target directory · {}",
            ui::format_duration(start.elapsed())
        ));
    } else {
        let config = workspace::config(&workspace_root);
        let selected = programs::select(&programs::listed(&config), requested);
        let packages: Vec<String> = selected
            .iter()
            .map(|name| generate::read_package_name(&programs::program_dir(&workspace_root, name)))
            .collect();

        ui::header(
            "clean",
            format!(
                "{} program{}",
                selected.len(),
                if selected.len() == 1 { "" } else { "s" }
            ),
        );

        let triples = solana_triples(&target_dir);
        let sbf_rustc = if !triples.is_empty() {
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
            Some(rustc)
        } else {
            None
        };

        for (prog_name, package) in selected.iter().zip(&packages) {
            let program_dir = programs::program_dir(&workspace_root, prog_name);
            let mode_label = if generate::depends_on_naclac_lang(&program_dir) {
                generate::detect_program_features(&workspace_root, &program_dir)
                    .unwrap_or_else(|e| ui::error(format!("Cannot clean '{prog_name}': {e}")))
                    .label()
            } else {
                "plain program".to_string()
            };

            ui::section(prog_name, mode_label);

            for release in [false, true] {
                let mut host = Command::new("cargo");
                host.arg("clean")
                    .arg("-p")
                    .arg(package)
                    .current_dir(&workspace_root);
                if release {
                    host.arg("--release");
                }
                run_clean_command(&mut host, "host build output removed");
            }

            if let Some(rustc) = &sbf_rustc {
                for triple in &triples {
                    for release in [false, true] {
                        let mut sbf = Command::new("cargo");
                        sbf.arg("clean")
                            .arg("--target")
                            .arg(triple)
                            .arg("-p")
                            .arg(package)
                            .env("RUSTC", rustc)
                            .current_dir(&workspace_root);
                        if release {
                            sbf.arg("--release");
                        }
                        run_clean_command(&mut sbf, "on-chain build output removed");
                    }
                }
            }

            ui::success("build output removed");
        }

        ui::milestone(format!(
            "Cleaned {} program{} · {}",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" },
            ui::format_duration(start.elapsed())
        ));
    }
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

fn run_clean_command(command: &mut Command, success_msg: &str) {
    if ui::show_output() {
        let status = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap_or_else(|e| ui::error(format!("Failed to run cargo clean: {e}")));
        if !status.success() {
            ui::error("cargo clean failed.");
        }
    } else {
        let mut child = command
            .envs(cargo_progress::ENV)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| ui::error(format!("Failed to spawn cargo clean: {e}")));

        let stderr = child
            .stderr
            .take()
            .unwrap_or_else(|| ui::error("Cannot read the output of cargo clean."));
        cargo_progress::read_updates(stderr, |line| {
            if !build::is_cargo_status_line(line) && !line.trim().is_empty() {
                eprintln!("{line}");
            }
        })
        .unwrap_or_else(|e| ui::error(format!("Cannot read the output of cargo clean: {e}")));

        let status = child
            .wait()
            .unwrap_or_else(|e| ui::error(format!("Failed to wait on cargo clean: {e}")));
        if !status.success() {
            ui::error(format!("cargo clean failed ({success_msg})."));
        }
    }
}
