use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use naclac_idl::Idl;
use std::fs;
use toml::Value;

pub fn execute(target_file: Option<&str>) {
    let current_dir = std::env::current_dir().unwrap();
    let toml_path = if current_dir.join("Naclac.toml").exists() {
        current_dir.join("Naclac.toml")
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir
            .join("../../Naclac.toml")
            .canonicalize()
            .unwrap()
    } else {
        eprintln!("❌ Not a Naclac workspace.");
        std::process::exit(1);
    };

    let content = fs::read_to_string(&toml_path).unwrap_or_default();
    let parsed: Value = toml::from_str(&content).unwrap_or(Value::Table(Default::default()));

    let cluster = parsed
        .get("provider")
        .and_then(|p| p.get("cluster"))
        .and_then(|c| c.as_str())
        .unwrap_or("localnet");

    let mut program_names = Vec::new();
    let mut program_ids = Vec::new();
    let mut found_table = None;

    if let Some(programs) = parsed.get("programs") {
        if let Some(tbl) = programs.get(cluster).and_then(|c| c.as_table()) {
            found_table = Some(tbl);
        } else if let Some(tbl) = programs.get("devnet").and_then(|c| c.as_table()) {
            found_table = Some(tbl);
        }
    }

    if let Some(tbl) = found_table {
        for (name, val) in tbl {
            program_names.push(name.clone());
            if let Some(pid) = val.as_str() {
                program_ids.push(pid.to_string());
            }
        }
    }

    if program_ids.is_empty() {
        eprintln!(
            "❌ Error: No programs found in Naclac.toml for cluster '{}'.",
            cluster
        );
        return;
    }

    // IDL files live next to the Naclac.toml (the workspace root), not the CWD
    let workspace_root = toml_path.parent().unwrap();

    // Load IDLs to match discriminators
    let mut idls = Vec::new();
    let idl_dir = workspace_root.join("target/idl");
    if idl_dir.exists() {
        for entry in fs::read_dir(&idl_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let idl_json = fs::read_to_string(path).unwrap();
                if let Ok(idl) = serde_json::from_str::<Idl>(&idl_json) {
                    idls.push(idl);
                }
            }
        }
    } else {
        eprintln!(
            "⚠️  No IDL directory found at {:?}. Instruction names will show as 'Unknown'.",
            idl_dir
        );
        eprintln!("   Run 'naclac build' first to generate the IDL.");
    }

    println!(
        "🚀 {} Profiling {} programs on {}",
        "NACLAC".bold().green(),
        program_ids.len(),
        cluster.cyan()
    );

    // 🌟 RUST PROGRAM TEST SEQUENTIAL EXECUTION 🌟
    let mut run_programs = Vec::new();
    if let Some(target) = target_file {
        if program_names.contains(&target.to_string()) {
            run_programs.push(target.to_string());
        } else {
            eprintln!(
                "❌ Error: Program '{}' not found in Naclac.toml active programs list.",
                target
            );
            std::process::exit(1);
        }
    } else {
        run_programs = program_names.clone();
    }

    if run_programs.is_empty() {
        eprintln!("❌ Error: No programs to profile.");
        return;
    }

    // Configure profiling log file path (absolute path to workspace root/target/naclac_profile.jsonl)
    let profile_file = workspace_root.join("target/naclac_profile.jsonl");
    let profile_file_str = profile_file.to_str().unwrap().to_string();

    let mut all_reports: Vec<(String, Vec<(String, u64)>)> = Vec::new();
    let mut overall_success = true;

    for prog_name in run_programs {
        // Clear/delete the old profile data file before running this program's tests
        if profile_file.exists() {
            let _ = fs::remove_file(&profile_file);
        }

        // Determine base Rust test script from Naclac.toml or default
        let mut test_script = String::new();
        for key in &["test-rust", "test"] {
            let key_str = format!("{} =", key);
            let mut found_script = String::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with(&key_str) {
                    if let Some(start) = trimmed.find('"') {
                        if let Some(end) = trimmed.rfind('"') {
                            if start != end {
                                found_script = trimmed[start + 1..end].to_string();
                                found_script = found_script.replace("\\\"", "\"");
                            }
                        }
                    }
                }
            }
            if !found_script.is_empty() && (key == &"test-rust" || found_script.contains("cargo")) {
                test_script = found_script;
                break;
            }
        }

        if test_script.is_empty() {
            test_script = format!(
                "cargo test --package {} --test integration -- --nocapture",
                prog_name
            );
        } else {
            // Inject program package
            if let Some(idx) = test_script.find(" -- ") {
                test_script.insert_str(idx, &format!(" --package {}", prog_name));
            } else {
                test_script = format!("{} --package {}", test_script, prog_name);
            }
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
            prog_name.white().bold()
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
            .arg(&test_script)
            .current_dir(workspace_root)
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
            eprintln!("❌ Test for program {} failed.", prog_name.red().bold());
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

        // Parse the generated jsonl file
        let mut raw_data = Vec::new();
        if profile_file.exists() {
            if let Ok(file_content) = fs::read_to_string(&profile_file) {
                for line in file_content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        let total_cu = val
                            .get("compute_units_consumed")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        let mut cu_per_invocation = Vec::new();
                        if let Some(logs_arr) = val.get("logs").and_then(|l| l.as_array()) {
                            for log_val in logs_arr {
                                if let Some(log_line) = log_val.as_str() {
                                    if log_line.contains("consumed")
                                        && log_line.contains("compute units")
                                    {
                                        let parts: Vec<&str> =
                                            log_line.split_whitespace().collect();
                                        if parts.len() >= 4 {
                                            if let Ok(n) = parts[3].parse::<u64>() {
                                                cu_per_invocation.push((parts[1].to_string(), n));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(ixs_arr) = val.get("instructions").and_then(|i| i.as_array()) {
                            let mut log_idx = 0usize;
                            for ix_val in ixs_arr {
                                let program_id = ix_val
                                    .get("program_id")
                                    .and_then(|p| p.as_str())
                                    .unwrap_or("");
                                let data_hex =
                                    ix_val.get("data").and_then(|d| d.as_str()).unwrap_or("");

                                while log_idx < cu_per_invocation.len()
                                    && cu_per_invocation[log_idx].0 != program_id
                                {
                                    log_idx += 1;
                                }

                                let cu = if log_idx < cu_per_invocation.len() {
                                    let c = cu_per_invocation[log_idx].1;
                                    log_idx += 1;
                                    c
                                } else {
                                    total_cu
                                };

                                let target_pid = if let Some(pos) =
                                    program_names.iter().position(|n| n == &prog_name)
                                {
                                    program_ids.get(pos).cloned().unwrap_or_default()
                                } else {
                                    String::new()
                                };

                                if program_id == target_pid {
                                    let mut instruction_name = "Unknown".to_string();
                                    if let Ok(data_bytes) = hex::decode(data_hex) {
                                        if data_bytes.len() >= 8 {
                                            let mut disc = [0u8; 8];
                                            disc.copy_from_slice(&data_bytes[0..8]);
                                            for idl in &idls {
                                                for idl_ix in &idl.instructions {
                                                    if idl_ix.discriminator == disc {
                                                        instruction_name = idl_ix.name.clone();
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    raw_data.push((instruction_name, cu));
                                }
                            }
                        }
                    }
                }
            }
        }

        if !raw_data.is_empty() {
            all_reports.push((prog_name.clone(), raw_data));
        } else {
            println!("⚠️  No transactions were captured for {}", prog_name);
        }
    }

    // Clean up profiling log file
    if profile_file.exists() {
        let _ = fs::remove_file(&profile_file);
    }

    println!("\n🏁 All tests complete. Printing final profiling data...\n");

    // Print reports
    for (test_file, report_data) in all_reports {
        println!("📝 {} {}", "Result for".magenta().bold(), test_file.white());
        print_final_summary(&report_data);
    }

    if !overall_success {
        eprintln!("\n❌ Profiling session finished with errors. Some tests failed.");
        std::process::exit(1);
    } else {
        println!("✅ Profiling session complete.");
    }
}

fn print_final_summary(data: &[(String, u64)]) {
    println!("\n{}", "📊 NACLAC FINAL COMPUTE REPORT".bold().green());
    println!(
        "{}",
        "────────────────────────────────────────────────────────────────────────────────".dimmed()
    );
    println!(
        "{:<30} {:<15} {:<15} {:<15}",
        "Instruction".bold(),
        "CU".bold(),
        "Est. Cost".bold(),
        "Visual".bold()
    );
    println!(
        "{}",
        "────────────────────────────────────────────────────────────────────────────────".dimmed()
    );

    for (name, cu) in data {
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
            "{:<30} {:<15} {:<15} {}{}",
            name.yellow(),
            format!("{} CU", cu).color(cu_color),
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
