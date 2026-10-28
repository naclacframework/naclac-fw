//! `naclac profile`: runs each program's Rust tests and reports the compute
//! units its instructions used. Everything it needs is checked before the first
//! test runs, and nothing in the recorded data is guessed: a record it cannot
//! read exactly is an error that names the record.

use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    colored::*,
    indicatif::{ProgressBar, ProgressStyle},
    naclac_idl::Idl,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

/// A program to profile: where its tests are sent, and what its
/// instructions are called, both from its own IDL.
struct Target {
    name: String,
    address: String,
    /// Instruction name and discriminator.
    instructions: Vec<(String, Vec<u8>)>,
}

fn load_target(idl_dir: &Path, name: &str) -> Target {
    let path = idl_dir.join(format!("{name}.json"));
    let json = fs::read_to_string(&path).unwrap_or_else(|e| {
        ui::error(format!(
            "Cannot read the IDL of '{name}' at {}: {e}. Run `naclac build` first (a program \
             that does not depend on naclac-lang has no IDL and cannot be profiled).",
            path.display()
        ))
    });
    let idl: Idl = serde_json::from_str(&json).unwrap_or_else(|e| {
        ui::error(format!(
            "The IDL of '{name}' at {} is not valid: {e}. Run `naclac build` to regenerate it.",
            path.display()
        ))
    });
    Target {
        name: name.to_string(),
        address: idl.address,
        instructions: idl
            .instructions
            .into_iter()
            .map(|ix| (ix.name, ix.discriminator))
            .collect(),
    }
}

fn clear_profile_file(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => ui::error(format!(
            "Cannot delete the old profile data {}: {e}",
            path.display()
        )),
    }
}

/// One program invocation in a transaction's logs.
struct Call {
    program: String,
    depth: usize,
    consumed: Option<u64>,
}

fn close_call(
    open: &mut Vec<usize>,
    calls: &[Call],
    program: &str,
    line: &str,
) -> Result<(), String> {
    let Some(index) = open.pop() else {
        return Err(format!(
            "log line `{line}` ends a program that never started"
        ));
    };
    if calls[index].program != program {
        return Err(format!(
            "log line `{line}` ends {program} while {} is the program running",
            calls[index].program
        ));
    }
    Ok(())
}

/// The program invocations a transaction's logs show, in order, with the
/// compute units each reported. Lines that are not about an invocation
/// (`Program log:`, `Program data:`, `Program return:`, anything else) carry
/// no invocation and are skipped.
fn parse_calls(logs: &[String]) -> Result<Vec<Call>, String> {
    let mut calls: Vec<Call> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for line in logs {
        if line.starts_with("Program log:")
            || line.starts_with("Program data:")
            || line.starts_with("Program return:")
        {
            continue;
        }
        let Some(rest) = line.strip_prefix("Program ") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let (Some(program), Some(event)) = (words.next(), words.next()) else {
            continue;
        };
        match event {
            "invoke" => {
                let depth = words
                    .next()
                    .and_then(|word| word.strip_prefix('['))
                    .and_then(|word| word.strip_suffix(']'))
                    .and_then(|word| word.parse::<usize>().ok())
                    .ok_or_else(|| format!("unreadable log line `{line}`"))?;
                if depth != open.len() + 1 {
                    return Err(format!(
                        "log line `{line}` starts a program at depth {depth} while {} program(s) \
                         are running",
                        open.len()
                    ));
                }
                open.push(calls.len());
                calls.push(Call {
                    program: program.to_string(),
                    depth,
                    consumed: None,
                });
            }
            "consumed" => {
                let units = words
                    .next()
                    .and_then(|word| word.parse::<u64>().ok())
                    .ok_or_else(|| format!("unreadable log line `{line}`"))?;
                let Some(&index) = open.last() else {
                    return Err(format!(
                        "log line `{line}` reports a program that is not running"
                    ));
                };
                if calls[index].program != program {
                    return Err(format!(
                        "log line `{line}` reports {program} while {} is the program running",
                        calls[index].program
                    ));
                }
                calls[index].consumed = Some(units);
            }
            "success" => close_call(&mut open, &calls, program, line)?,
            failed if failed.starts_with("failed") => close_call(&mut open, &calls, program, line)?,
            _ => {}
        }
    }
    if !open.is_empty() {
        return Err(
            "the logs end while a program is still running; long logs are cut off at the \
             runtime's log limit"
                .to_string(),
        );
    }
    Ok(calls)
}

/// A recorded transaction: its instructions (program, data) and its logs.
struct Record {
    instructions: Vec<(String, Vec<u8>)>,
    logs: Vec<String>,
}

fn parse_record(line: &str) -> Result<Record, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("not valid JSON: {e}"))?;
    let instructions = value
        .get("instructions")
        .and_then(|v| v.as_array())
        .ok_or("the record has no `instructions` list")?
        .iter()
        .map(|ix| -> Result<(String, Vec<u8>), String> {
            let program = ix
                .get("program_id")
                .and_then(|v| v.as_str())
                .ok_or("an instruction has no `program_id`")?;
            let data = ix
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or("an instruction has no `data`")?;
            let data = hex::decode(data)
                .map_err(|e| format!("an instruction's `data` is not hex: {e}"))?;
            Ok((program.to_string(), data))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let logs = value
        .get("logs")
        .and_then(|v| v.as_array())
        .ok_or("the record has no `logs` list")?
        .iter()
        .map(|log| -> Result<String, String> {
            Ok(log
                .as_str()
                .ok_or("a log entry is not a string")?
                .to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Record { instructions, logs })
}

/// The name of the instruction whose discriminator starts `data`, among the
/// program's own.
fn instruction_name(
    instructions: &[(String, Vec<u8>)],
    data: &[u8],
    program: &str,
) -> Result<String, String> {
    let matches: Vec<&str> = instructions
        .iter()
        .filter(|(_, discriminator)| !discriminator.is_empty() && data.starts_with(discriminator))
        .map(|(name, _)| name.as_str())
        .collect();
    match matches.as_slice() {
        [name] => Ok((*name).to_string()),
        [] => Err(format!(
            "an instruction's data starts with {} which matches no instruction in the IDL of \
             '{program}'; the IDL is out of date, run `naclac build`",
            hex::encode(&data[..data.len().min(8)])
        )),
        _ => Err(format!(
            "an instruction's data starts with {} which matches several instructions in the \
             IDL of '{program}': {}",
            hex::encode(&data[..data.len().min(8)]),
            matches.join(", ")
        )),
    }
}

/// The compute units of each of `target`'s instructions in one transaction,
/// named by its own IDL.
fn profile_record(record: &Record, target: &Target) -> Result<Vec<(String, u64)>, String> {
    let calls = parse_calls(&record.logs)?;
    let top: Vec<&Call> = calls.iter().filter(|call| call.depth == 1).collect();
    if top.len() != record.instructions.len() {
        return Err(format!(
            "the transaction has {} instruction(s) but its logs show {} top-level program \
             invocation(s)",
            record.instructions.len(),
            top.len()
        ));
    }
    let mut rows = Vec::new();
    for (index, ((program, data), call)) in record.instructions.iter().zip(&top).enumerate() {
        if &call.program != program {
            return Err(format!(
                "instruction {index} is for {program} but the logs show {}",
                call.program
            ));
        }
        if program != &target.address {
            continue;
        }
        let units = call.consumed.ok_or_else(|| {
            format!("no compute-unit line in the logs for instruction {index} of {program}")
        })?;
        rows.push((
            instruction_name(&target.instructions, data, &target.name)?,
            units,
        ));
    }
    Ok(rows)
}

fn fail_at(path: &Path, index: usize, problem: &str) -> ! {
    ui::error(format!("{}:{}: {problem}", path.display(), index + 1))
}

/// Every instruction of `target` the tests recorded.
fn read_profile(path: &Path, target: &Target) -> Vec<(String, u64)> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => ui::error(format!(
            "Cannot read the profile data {}: {e}",
            path.display()
        )),
    };
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = parse_record(line).unwrap_or_else(|e| fail_at(path, index, &e));
        rows.extend(profile_record(&record, target).unwrap_or_else(|e| fail_at(path, index, &e)));
    }
    rows
}

pub fn execute(requested: &[String], run_order: bool) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let names = programs::select(&programs::listed(&config), requested);
    let test_script = workspace::rust_test_script(&config);

    let idl_dir = naclac_client_gen::idl_dir(&workspace_root);
    let targets: Vec<Target> = names
        .iter()
        .map(|name| load_target(&idl_dir, name))
        .collect();

    let profile_file: PathBuf =
        naclac_client_gen::artifacts_dir(&workspace_root).join("naclac_profile.jsonl");
    clear_profile_file(&profile_file);
    let profile_file_str = profile_file.to_str().unwrap().to_string();

    println!(
        "🚀 {} Profiling {} program(s)",
        "NACLAC".bold().green(),
        targets.len()
    );

    let mut all_reports: Vec<(String, Vec<(String, u64)>)> = Vec::new();
    let mut overall_success = true;

    for target in &targets {
        clear_profile_file(&profile_file);

        let mut script = test_script.clone();
        if let Some(idx) = script.find(" -- ") {
            script.insert_str(idx, &format!(" --package {}", target.name));
        } else {
            script = format!("{} --package {}", script, target.name);
        }

        // Display a clean, custom spinner using indicatif
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(format!(
            "Profiling program: {} (running tests)...",
            target.name.white().bold()
        ));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

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

        let child = std::process::Command::new(shell)
            .arg(shell_flag)
            .arg(&script)
            .current_dir(&workspace_root)
            .env("NACLAC_PROFILE_FILE", &profile_file_str) // Inject log file path!
            .stdout(std::process::Stdio::piped()) // Capture stdout
            .stderr(std::process::Stdio::piped()) // Capture stderr
            .spawn()
            .expect("Failed to execute test script");

        let output = child
            .wait_with_output()
            .expect("Failed to wait on test execution");
        pb.finish_and_clear();

        if !output.status.success() {
            eprintln!("❌ Test for program {} failed.", target.name.red().bold());
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if !stdout_str.is_empty() {
                eprintln!("\n📋 Test stdout:\n{}", stdout_str);
            }
            if !stderr_str.is_empty() {
                eprintln!("\n📋 Test stderr:\n{}", stderr_str);
            }
            overall_success = false;
            break;
        }

        let rows = read_profile(&profile_file, target);
        if rows.is_empty() {
            ui::error(format!(
                "No transaction to '{}' was recorded. Its tests must send transactions through \
                 naclac-client's provider.",
                target.name
            ));
        }
        all_reports.push((target.name.clone(), rows));
    }

    clear_profile_file(&profile_file);

    println!("\n🏁 All tests complete. Printing final profiling data...\n");

    // Print reports
    for (test_file, report_data) in all_reports {
        println!("📝 {} {}", "Result for".magenta().bold(), test_file.white());
        print_final_summary(&report_data, run_order);
    }

    if !overall_success {
        eprintln!("\n❌ Profiling session finished with errors. Some tests failed.");
        std::process::exit(1);
    } else {
        println!("✅ Profiling session complete.");
    }
}

fn print_final_summary(data: &[(String, u64)], run_order: bool) {
    let mut grouped: Vec<(String, u64, usize)> = Vec::new();
    for (name, cu) in data {
        if let Some(entry) = grouped.iter_mut().find(|(n, c, _)| n == name && c == cu) {
            entry.2 += 1;
        } else {
            grouped.push((name.clone(), *cu, 1));
        }
    }
    if !run_order {
        grouped.sort_by_key(|(_, cu, _)| *cu);
    }

    println!("\n{}", "📊 NACLAC FINAL COMPUTE REPORT".bold().green());
    println!(
        "{}",
        "────────────────────────────────────────────────────────────────────────────────".dimmed()
    );
    println!(
        "{:<30} {:<15} {:<10} {:<15} {:<15}",
        "Instruction".bold(),
        "CU".bold(),
        "Calls".bold(),
        "Est. Cost".bold(),
        "Visual".bold()
    );
    println!(
        "{}",
        "────────────────────────────────────────────────────────────────────────────────".dimmed()
    );

    for (name, cu, calls) in &grouped {
        let cu_color = if *cu > 180_000 {
            Color::Red
        } else if *cu > 100_000 {
            Color::Yellow
        } else if *cu > 50_000 {
            Color::Cyan
        } else {
            Color::Green
        };

        let cost_sol = *cu as f64 * 0.000000001;

        let bar_len = (*cu as f64 / 200_000.0 * 20.0).min(20.0) as usize;
        let bar = "█".repeat(bar_len);
        let empty = "░".repeat(20 - bar_len);

        println!(
            "{:<30} {:<15} {:<10} {:<15} {}{}",
            name.yellow(),
            format!("{} CU", cu).color(cu_color),
            format!("×{}", calls),
            format!("{:.7} SOL", cost_sol).dimmed(),
            bar.color(cu_color),
            empty.dimmed()
        );
    }
    println!(
        "{}",
        "────────────────────────────────────────────────────────────────────────────────".dimmed()
    );
}

#[cfg(test)]
mod tests {
    use super::{instruction_name, parse_calls, parse_record, profile_record, Target};

    fn logs(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    fn target() -> Target {
        Target {
            name: "escrow".to_string(),
            address: "Prog".to_string(),
            instructions: vec![
                ("make".to_string(), vec![0]),
                ("take".to_string(), vec![2, 7]),
            ],
        }
    }

    #[test]
    fn calls_come_out_in_order_with_their_own_units() {
        let calls = parse_calls(&logs(&[
            "Program Prog invoke [1]",
            "Program log: Program Other invoke [1]",
            "Program Inner invoke [2]",
            "Program Inner consumed 105 of 190000 compute units",
            "Program Inner success",
            "Program Prog consumed 3591 of 200000 compute units",
            "Program Prog success",
        ]))
        .unwrap();
        let shown: Vec<(&str, usize, Option<u64>)> = calls
            .iter()
            .map(|call| (call.program.as_str(), call.depth, call.consumed))
            .collect();
        assert_eq!(shown, [("Prog", 1, Some(3591)), ("Inner", 2, Some(105))]);
    }

    #[test]
    fn logs_that_end_inside_a_program_are_an_error() {
        let error = parse_calls(&logs(&["Program Prog invoke [1]", "Log truncated"]))
            .err()
            .unwrap();
        assert!(error.contains("log limit"), "{error}");
    }

    #[test]
    fn a_depth_that_does_not_follow_the_running_programs_is_an_error() {
        let error = parse_calls(&logs(&["Program Prog invoke [2]"]))
            .err()
            .unwrap();
        assert!(error.contains("depth 2"), "{error}");
    }

    #[test]
    fn a_consumed_line_for_another_program_is_an_error() {
        let error = parse_calls(&logs(&[
            "Program Prog invoke [1]",
            "Program Other consumed 5 of 9 compute units",
        ]))
        .err()
        .unwrap();
        assert!(error.contains("Other"), "{error}");
    }

    #[test]
    fn an_instruction_is_named_by_its_own_discriminator_only() {
        let target = target();
        assert_eq!(
            instruction_name(&target.instructions, &[2, 7, 9], "escrow").unwrap(),
            "take"
        );
        let unknown = instruction_name(&target.instructions, &[9, 9], "escrow")
            .err()
            .unwrap();
        assert!(unknown.contains("naclac build"), "{unknown}");
    }

    #[test]
    fn a_transaction_gives_each_target_instruction_its_own_units() {
        let record = parse_record(
            r#"{"instructions":[{"program_id":"System","data":"00"},{"program_id":"Prog","data":"00"},{"program_id":"Prog","data":"0207"}],
                "logs":["Program System invoke [1]","Program System success",
                        "Program Prog invoke [1]","Program Prog consumed 100 of 200 compute units","Program Prog success",
                        "Program Prog invoke [1]","Program Prog consumed 250 of 200 compute units","Program Prog success"]}"#,
        )
        .unwrap();
        let rows = profile_record(&record, &target()).unwrap();
        assert_eq!(rows, [("make".to_string(), 100), ("take".to_string(), 250)]);
    }

    #[test]
    fn a_target_instruction_without_a_units_line_is_an_error() {
        let record = parse_record(
            r#"{"instructions":[{"program_id":"Prog","data":"00"}],
                "logs":["Program Prog invoke [1]","Program Prog success"]}"#,
        )
        .unwrap();
        let error = profile_record(&record, &target()).err().unwrap();
        assert!(error.contains("no compute-unit line"), "{error}");
    }

    #[test]
    fn a_record_whose_logs_and_instructions_disagree_is_an_error() {
        let record = parse_record(
            r#"{"instructions":[{"program_id":"Prog","data":"00"},{"program_id":"Prog","data":"00"}],
                "logs":["Program Prog invoke [1]","Program Prog consumed 1 of 2 compute units","Program Prog success"]}"#,
        )
        .unwrap();
        let error = profile_record(&record, &target()).err().unwrap();
        assert!(error.contains("2 instruction(s)"), "{error}");
    }

    #[test]
    fn a_record_missing_a_field_is_an_error() {
        assert!(parse_record(r#"{"logs":[]}"#).is_err());
        assert!(parse_record(r#"{"instructions":[],"logs":[1]}"#).is_err());
        assert!(parse_record("not json").is_err());
    }
}
