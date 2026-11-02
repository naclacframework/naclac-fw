use {
    crate::{
        cargo_progress,
        commands::{build, generate, programs, workspace},
        ui,
    },
    colored::Colorize,
    std::{
        path::PathBuf,
        process::{Command, Stdio},
        time::Instant,
    },
};

fn clippy_spawn_error(error: &std::io::Error) -> String {
    format!(
        "Cannot run cargo clippy: {error}. Make sure cargo and clippy are installed: \
         rustup component add clippy"
    )
}

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

    generate::ensure_client_stubs(&workspace_root, &programs::all_dirs(&workspace_root));

    let start = Instant::now();
    let target_label = if tests {
        format!(
            "tests of {} program{}",
            program_dirs.len(),
            if program_dirs.len() == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "{} program{}",
            program_dirs.len(),
            if program_dirs.len() == 1 { "" } else { "s" }
        )
    };
    ui::header("check", target_label);
    if deny_warnings {
        ui::kv("flags", "-D warnings (warnings treated as errors)");
    }

    for program_dir in &program_dirs {
        let prog_name = programs::dir_name(program_dir);
        let prog_name = prog_name.as_str();
        let cargo_toml_path = program_dir.join("Cargo.toml");

        let mode_label = if generate::depends_on_naclac_lang(program_dir) {
            generate::detect_program_features(&workspace_root, program_dir)
                .unwrap_or_else(|e| ui::error(format!("Cannot check '{prog_name}': {e}")))
                .label()
        } else {
            "plain program".to_string()
        };

        ui::section(
            prog_name,
            if tests {
                format!("tests · {}", mode_label)
            } else {
                mode_label
            },
        );

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--manifest-path")
            .arg(&cargo_toml_path)
            .env("CARGO_TERM_COLOR", "always")
            .current_dir(&workspace_root)
            .stdout(Stdio::inherit());

        if tests {
            cmd.arg("--tests");
        }
        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        }
        if deny_warnings {
            cmd.args(["--", "-D", "warnings"]);
        }

        let spinner_msg = if tests {
            "Checking tests with clippy"
        } else {
            "Checking program with clippy"
        };

        if ui::show_output() {
            let status = cmd
                .stderr(Stdio::inherit())
                .status()
                .unwrap_or_else(|e| ui::error(clippy_spawn_error(&e)));
            if !status.success() {
                if tests {
                    ui::error(format!(
                        "Clippy check failed for the tests of '{prog_name}'. If it could not resolve the \
                         generated client, run `naclac build` first."
                    ));
                } else {
                    ui::error(format!("Clippy check failed for '{prog_name}'."));
                }
            }
            ui::success(if tests {
                "tests clippy clean"
            } else {
                "clippy clean"
            });
        } else {
            let mut child = cmd
                .envs(cargo_progress::ENV)
                .stderr(Stdio::piped())
                .spawn()
                .unwrap_or_else(|e| ui::error(clippy_spawn_error(&e)));
            let stderr = child
                .stderr
                .take()
                .unwrap_or_else(|| ui::error("Cannot read the output of cargo clippy."));

            let step = ui::Step::start(spinner_msg);
            let mut printed_output_header = false;
            let mut cargo_error_summary: Option<String> = None;

            let read = cargo_progress::read_updates(stderr, |line| {
                if let Some(progress) = cargo_progress::parse_progress(line) {
                    step.set_message(format!(
                        "{spinner_msg}  {}",
                        cargo_progress::render(&progress)
                    ));
                    return;
                }
                if build::is_cargo_status_line(line) || line.trim().is_empty() {
                    return;
                }
                if !printed_output_header {
                    step.print_above("── compiler output ──────────────────────".dimmed());
                    printed_output_header = true;
                }
                let stripped = build::strip_ansi(line);
                let trimmed = stripped.trim();
                if trimmed.starts_with("error: could not compile")
                    || trimmed.starts_with("error: aborting due to")
                {
                    cargo_error_summary = Some(trimmed.to_string());
                }
                step.print_above(line);
            });

            if let Err(e) = read {
                step.fail(format!("Cannot read the output of cargo clippy: {e}"));
            }

            let status = child
                .wait()
                .unwrap_or_else(|e| ui::error(format!("Cannot wait for cargo clippy: {e}")));
            if !status.success() {
                let fail_msg = if tests {
                    match cargo_error_summary {
                        Some(summary) => format!(
                            "Clippy check failed for the tests of '{prog_name}' — {summary}"
                        ),
                        None => format!(
                            "Clippy check failed for the tests of '{prog_name}'. If it could not \
                             resolve the generated client, run `naclac build` first."
                        ),
                    }
                } else {
                    match cargo_error_summary {
                        Some(summary) => {
                            format!("Clippy check failed for '{prog_name}' — {summary}")
                        }
                        None => format!("Clippy check failed for '{prog_name}'"),
                    }
                };
                step.fail(fail_msg);
            }

            step.done(if tests {
                "tests clippy clean"
            } else {
                "clippy clean"
            });
        }
    }

    ui::milestone(format!(
        "Checked {} program{} · {}",
        program_dirs.len(),
        if program_dirs.len() == 1 { "" } else { "s" },
        ui::format_duration(start.elapsed())
    ));
}
