use {
    crate::{
        cargo_progress,
        commands::{build, generate, programs, workspace},
        ui,
    },
    colored::Colorize,
    std::{
        fs,
        io::Read,
        path::PathBuf,
        process::{Command, Stdio},
        time::Instant,
    },
};

/// Expands the program (desugars all macros into pure Rust) for each requested
/// program (or all listed programs in Naclac.toml when none specified), writing
/// the expanded code into `programs/<name>/<name>.rs` (or a custom file via
/// `-o / --output`).
pub fn execute(requested: &[String], features: Vec<String>, output_target: Option<&str>) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let listed = programs::listed(&config);
    let selected = programs::select(&listed, requested);

    if selected.len() > 1 && output_target.is_some() {
        ui::error(
            "`-o / --output` can only be used when expanding a single program with `-p <program>`.",
        );
    }

    let start = Instant::now();
    ui::header(
        "expand",
        format!(
            "{} program{}",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" }
        ),
    );

    for program_name in &selected {
        let program_dir = programs::program_dir(&workspace_root, program_name);
        let cargo_toml_path = program_dir.join("Cargo.toml");

        let mode_label = if generate::depends_on_naclac_lang(&program_dir) {
            generate::detect_program_features(&workspace_root, &program_dir)
                .unwrap_or_else(|e| ui::error(format!("Cannot expand '{program_name}': {e}")))
                .label()
        } else {
            "plain program".to_string()
        };

        let out_path = match output_target {
            Some(target) => {
                let p = PathBuf::from(target);
                if p.is_absolute() {
                    p
                } else {
                    workspace_root.join(p)
                }
            }
            None => program_dir.join(format!("{program_name}.rs")),
        };

        let display_path = out_path
            .strip_prefix(&workspace_root)
            .unwrap_or(&out_path)
            .display()
            .to_string();

        ui::section(program_name, mode_label);
        ui::kv("output", &display_path);
        if out_path.exists() {
            ui::warn(format!(
                "{display_path} already exists and will be overwritten"
            ));
        }
        if !features.is_empty() {
            ui::kv("features", features.join(", "));
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("expand")
            .arg("--manifest-path")
            .arg(&cargo_toml_path)
            .current_dir(&workspace_root);

        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        }

        let step = ui::Step::start("Expanding program");

        let mut child = match cmd
            .envs(cargo_progress::ENV)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => step.fail(format!(
                "Cannot run cargo expand: {e}. Make sure cargo-expand is installed: \
                 cargo install cargo-expand"
            )),
        };

        let (Some(mut stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            step.fail("Cannot read the output of cargo expand.");
        };

        let stdout_thread = std::thread::spawn(move || {
            let mut buffer = String::new();
            stdout.read_to_string(&mut buffer).map(|_| buffer)
        });

        let mut printed_header = false;
        let read = cargo_progress::read_updates(stderr, |line| {
            if let Some(progress) = cargo_progress::parse_progress(line) {
                step.set_message(format!(
                    "Expanding program  {}",
                    cargo_progress::render(&progress)
                ));
                return;
            }
            if build::is_cargo_status_line(line) || line.trim().is_empty() {
                return;
            }
            if !printed_header {
                step.print_above("── compiler output ──────────────────────".dimmed());
                printed_header = true;
            }
            step.print_above(line);
        });

        if let Err(e) = read {
            step.fail(format!("Cannot read the output of cargo expand: {e}"));
        }
        let status = match child.wait() {
            Ok(status) => status,
            Err(e) => step.fail(format!("Cannot wait for cargo expand: {e}")),
        };
        let expanded_code = match stdout_thread.join() {
            Ok(Ok(code)) => code,
            Ok(Err(e)) => step.fail(format!("Cannot read the expanded code: {e}")),
            Err(_) => step.fail("Reading the expanded code stopped unexpectedly."),
        };

        if !status.success() || expanded_code.trim().is_empty() {
            step.fail(format!("Cargo expand failed for '{program_name}'."));
        }

        if let Some(parent) = out_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                step.fail(format!("Cannot create {}: {e}", parent.display()));
            }
        }
        if let Err(e) = fs::write(&out_path, &expanded_code) {
            step.fail(format!("Cannot write {}: {e}", out_path.display()));
        }

        step.done(format!("written to {display_path}"));
    }

    ui::milestone(format!(
        "Expanded {} program{} · {}",
        selected.len(),
        if selected.len() == 1 { "" } else { "s" },
        ui::format_duration(start.elapsed())
    ));
}
