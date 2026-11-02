use {
    crate::{
        cargo_progress,
        commands::{build, generate, programs, workspace},
        ui,
    },
    colored::Colorize,
    std::{
        process::{Command, Stdio},
        time::Instant,
    },
};

fn test_spawn_error(error: &std::io::Error) -> String {
    format!("Cannot run the test script: {error}")
}

pub fn execute(target_file: Option<&str>, requested: &[String], rust: bool, node: bool, all: bool) {
    let run_node = node && !rust;

    if all {
        crate::commands::build::execute(requested, Vec::new(), false, Vec::new());
        crate::commands::deploy::execute(requested, true, false);
    }

    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);

    let mut test_script: String;
    let mut run_scripts = Vec::new();
    let selected_programs: Vec<String>;

    if run_node {
        if !requested.is_empty() {
            ui::error("`-p` selects programs for the Rust tests; the Node tests do not take it.");
        }
        test_script = workspace::node_test_script(&config);
        selected_programs = Vec::new();

        if let Some(file_name) = target_file {
            let target_path = if file_name.starts_with("tests/") {
                file_name.to_string()
            } else {
                format!("tests/{}", file_name)
            };

            if test_script.contains("\"tests/**/*.ts\"") {
                test_script =
                    test_script.replace("\"tests/**/*.ts\"", &format!("\"{}\"", target_path));
            } else if test_script.contains("tests/**/*.ts") {
                test_script = test_script.replace("tests/**/*.ts", &target_path);
            } else {
                test_script = format!("{} \"{}\"", test_script, target_path);
            }
        }
    } else {
        test_script = workspace::rust_test_script(&config);

        if let Some(f) = target_file {
            let mut clean_f = f.to_string();
            if clean_f.starts_with("tests/") {
                clean_f = clean_f["tests/".len()..].to_string();
            }
            if clean_f.ends_with(".rs") {
                clean_f = clean_f[..clean_f.len() - 3].to_string();
            }

            if let Some(idx) = test_script.find(" --test ") {
                let after_test = &test_script[idx + " --test ".len()..];
                let test_target = after_test.split_whitespace().next().unwrap_or_else(|| {
                    ui::error(format!(
                        "The Rust test script `{test_script}` has `--test` with no target after it."
                    ))
                });
                test_script = test_script.replace(test_target, &clean_f);
            } else if let Some(idx) = test_script.find(" -- ") {
                test_script.insert_str(idx, &format!(" --test {}", clean_f));
            } else {
                test_script = format!("{} --test {}", test_script, clean_f);
            }
        }

        let listed = programs::listed(&config);
        selected_programs = programs::select(&listed, requested);
        for p in &selected_programs {
            let mut s = test_script.clone();
            if let Some(idx) = s.find(" -- ") {
                s.insert_str(idx, &format!(" --package {}", p));
            } else {
                s = format!("{} --package {}", s, p);
            }
            run_scripts.push(s);
        }
    }

    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let shell_flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let start = Instant::now();
    let header_target = if run_node {
        "node".to_string()
    } else {
        format!(
            "{} program{}",
            selected_programs.len(),
            if selected_programs.len() == 1 {
                ""
            } else {
                "s"
            }
        )
    };
    ui::header("test", header_target);
    if let Some(file_name) = target_file {
        ui::kv("target", file_name);
    }

    if run_node {
        ui::section("node", "typescript test suite");

        let mut command = Command::new(shell);
        command
            .arg(shell_flag)
            .arg(&test_script)
            .current_dir(&workspace_root)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .env("TS_NODE_TRANSPILE_ONLY", "1");

        let step_start = Instant::now();
        let status = command
            .status()
            .unwrap_or_else(|e| ui::error(test_spawn_error(&e)));

        if !status.success() {
            ui::error("Node test suite failed.");
        }

        eprintln!(
            "  {} {} all tests passed",
            "✓".green().bold(),
            format!("{:>6}", ui::format_duration(step_start.elapsed())).dimmed()
        );

        ui::milestone(format!(
            "Node test suite passed · {}",
            ui::format_duration(start.elapsed())
        ));
    } else {
        for (prog_name, script) in selected_programs.iter().zip(&run_scripts) {
            let program_dir = programs::program_dir(&workspace_root, prog_name);
            let mode_label = if generate::depends_on_naclac_lang(&program_dir) {
                generate::detect_program_features(&workspace_root, &program_dir)
                    .unwrap_or_else(|e| ui::error(format!("Cannot test '{prog_name}': {e}")))
                    .label()
            } else {
                "plain program".to_string()
            };

            ui::section(prog_name, mode_label);

            let mut command = Command::new(shell);
            command
                .arg(shell_flag)
                .arg(script)
                .env("CARGO_TERM_COLOR", "always")
                .current_dir(&workspace_root);

            if ui::show_output() {
                let step_start = Instant::now();
                let status = command
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()
                    .unwrap_or_else(|e| ui::error(test_spawn_error(&e)));
                if !status.success() {
                    ui::error(format!("Tests failed for '{prog_name}'."));
                }
                eprintln!(
                    "  {} {} all tests passed",
                    "✓".green().bold(),
                    format!("{:>6}", ui::format_duration(step_start.elapsed())).dimmed()
                );
            } else {
                let mut child = command
                    .envs(cargo_progress::ENV)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| ui::error(test_spawn_error(&e)));

                let stderr = child
                    .stderr
                    .take()
                    .unwrap_or_else(|| ui::error("Cannot read the output of the test script."));
                let step = ui::Step::start("Compiling & running tests");

                let mut printed_header = false;
                let read = cargo_progress::read_updates(stderr, |line| {
                    if let Some(progress) = cargo_progress::parse_progress(line) {
                        step.set_message(format!(
                            "Compiling & running tests  {}",
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
                    step.fail(format!("Cannot read the output of the test script: {e}"));
                }

                let status = match child.wait() {
                    Ok(status) => status,
                    Err(e) => step.fail(format!("Cannot wait for the test script: {e}")),
                };

                if !status.success() {
                    step.fail(format!("Tests failed for '{prog_name}'."));
                }

                step.done("all tests passed");
            }
        }

        ui::milestone(format!(
            "Passed tests for {} program{} · {}",
            selected_programs.len(),
            if selected_programs.len() == 1 {
                ""
            } else {
                "s"
            },
            ui::format_duration(start.elapsed())
        ));
    }
}
