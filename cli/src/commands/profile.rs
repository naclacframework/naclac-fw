use std::collections::HashSet;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use toml::Value;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::CommitmentConfig;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_signature::Signature;
use solana_address::Address;
use std::str::FromStr;
use colored::*;
use naclac_idl::Idl;
use solana_transaction_status::UiTransactionEncoding;

pub fn execute(target_file: Option<&str>) {
    let current_dir = std::env::current_dir().unwrap();
    let toml_path = if current_dir.join("Naclac.toml").exists() {
        current_dir.join("Naclac.toml")
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../../Naclac.toml").canonicalize().unwrap()
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

    let rpc_url = match cluster {
        "localnet" => "http://localhost:8899",
        "devnet" => "https://api.devnet.solana.com",
        "testnet" => "https://api.testnet.solana.com",
        _ => cluster,
    };

    let mut program_ids = Vec::new();
    if let Some(programs_table) = parsed.get("programs").and_then(|p| p.get(cluster)).and_then(|c| c.as_table()) {
        for val in programs_table.values() {
            if let Some(pid) = val.as_str() {
                program_ids.push(pid.to_string());
            }
        }
    }

    if program_ids.is_empty() {
        eprintln!("❌ No programs found in Naclac.toml for cluster '{}'", cluster);
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
        eprintln!("⚠️  No IDL directory found at {:?}. Instruction names will show as 'Unknown'.", idl_dir);
        eprintln!("   Run 'naclac build' first to generate the IDL.");
    }

    println!("🚀 {} Profiling {} on {}", "NACLAC".bold().green(), program_ids.len(), cluster.cyan());
    println!("📡 Listening for transactions (commitment: confirmed)...");

    let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let processed_signatures = Arc::new(Mutex::new(HashSet::<String>::new()));
    let profile_data = Arc::new(Mutex::new(Vec::<(String, u64)>::new()));
    let tests_finished = Arc::new(AtomicBool::new(false));
    
    // Seed the processed signatures so we don't profile old history
    for pid_str in &program_ids {
        if let Ok(pid) = Address::from_str(pid_str) {
            if let Ok(sigs) = client.get_signatures_for_address(&pid) {
                let mut processed = processed_signatures.lock().unwrap();
                for sig_info in sigs {
                    processed.insert(sig_info.signature);
                }
            } else {
                println!("⚠️  Warning: Could not connect to RPC to seed signatures. Profiling might show duplicates.");
            }
        }
    }

    let processed_clone = Arc::clone(&processed_signatures);
    let pids_clone = program_ids.clone();
    let idls_clone = idls.clone();
    let rpc_url_clone = rpc_url.to_string();
    let profile_data_clone = Arc::clone(&profile_data);
    let finished_clone = Arc::clone(&tests_finished);

    // Spawn Observer Thread
    thread::spawn(move || {
        let client = RpcClient::new_with_commitment(rpc_url_clone, CommitmentConfig::confirmed());
        loop {
            for pid_str in &pids_clone {
                if let Ok(pid) = Address::from_str(pid_str) {
                    if let Ok(sigs) = client.get_signatures_for_address(&pid) {
                        for sig_info in sigs.into_iter().rev() {
                            let mut processed = processed_clone.lock().unwrap();
                            if !processed.contains(&sig_info.signature) {
                                processed.insert(sig_info.signature.clone());
                                drop(processed); // Release lock early
                                
                                if let Ok(sig) = Signature::from_str(&sig_info.signature) {
                                    let config = RpcTransactionConfig {
                                        encoding: Some(UiTransactionEncoding::Base64),
                                        commitment: Some(CommitmentConfig::confirmed()),
                                        max_supported_transaction_version: Some(0),
                                    };

                                    match client.get_transaction_with_config(&sig, config) {
                                        Ok(tx) => {
                                            if let Some(meta) = tx.transaction.meta {
                                                // Extract total CU as a plain u64 before any moves.
                                                let total_cu: u64 = match &meta.compute_units_consumed {
                                                    solana_transaction_status::option_serializer::OptionSerializer::Some(v) => *v,
                                                    _ => 0,
                                                };

                                                // Build a list of (program_id_str, cu_consumed) from
                                                // runtime log lines of the form:
                                                //   "Program <pid> consumed N of M compute units"
                                                // This gives us the true per-instruction CU cost.
                                                let mut cu_per_invocation: Vec<(String, u64)> = Vec::new();
                                                if let solana_transaction_status::option_serializer::OptionSerializer::Some(logs) = &meta.log_messages {
                                                    for log_line in logs.iter() {
                                                        if log_line.contains("consumed") && log_line.contains("compute units") {
                                                            let parts: Vec<&str> = log_line.split_whitespace().collect();
                                                            // ["Program", "<pid>", "consumed", "N", "of", "M", "compute", "units"]
                                                            if parts.len() >= 4 {
                                                                if let Ok(n) = parts[3].parse::<u64>() {
                                                                    cu_per_invocation.push((parts[1].to_string(), n));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                if let Some(ui_tx) = tx.transaction.transaction.decode() {
                                                    let message = &ui_tx.message;
                                                    let static_keys = message.static_account_keys();

                                                    // Walk instructions in the same order as log entries
                                                    // and match each to its corresponding log CU entry.
                                                    let mut log_idx = 0usize;

                                                    for ix in message.instructions() {
                                                        let program_id_index = ix.program_id_index as usize;
                                                        if program_id_index >= static_keys.len() {
                                                            continue;
                                                        }
                                                        let prog_id = static_keys[program_id_index];
                                                        let pid_str = prog_id.to_string();

                                                        // Advance to the next log entry matching this program.
                                                        while log_idx < cu_per_invocation.len()
                                                            && cu_per_invocation[log_idx].0 != pid_str
                                                        {
                                                            log_idx += 1;
                                                        }

                                                        // Use the log-derived CU; fall back to total if missing.
                                                        let cu = if log_idx < cu_per_invocation.len() {
                                                            let c = cu_per_invocation[log_idx].1;
                                                            log_idx += 1;
                                                            c
                                                        } else {
                                                            total_cu
                                                        };

                                                        if prog_id == pid {
                                                            let mut instruction_name = "Unknown".to_string();
                                                            if ix.data.len() >= 8 {
                                                                let mut disc = [0u8; 8];
                                                                disc.copy_from_slice(&ix.data[0..8]);
                                                                for idl in &idls_clone {
                                                                    for idl_ix in &idl.instructions {
                                                                        if idl_ix.discriminator == disc {
                                                                            instruction_name = idl_ix.name.clone();
                                                                            break;
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            let mut data = profile_data_clone.lock().unwrap();
                                                            data.push((instruction_name, cu));
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                        Err(_) => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
            if finished_clone.load(Ordering::SeqCst) {
                // One last check for remaining transactions
                thread::sleep(Duration::from_millis(1000));
                break;
            }
        }
    });

    // 🌟 TEST DISCOVERY & SEQUENTIAL EXECUTION 🌟
    let mut test_files = Vec::new();
    if let Some(file) = target_file {
        let target_path = if file.starts_with("tests/") {
            file.to_string()
        } else {
            format!("tests/{}", file)
        };
        test_files.push(target_path);
    } else {
        // Find all .ts files in the tests directory
        let tests_dir = workspace_root.join("tests");
        if tests_dir.exists() {
            for entry in fs::read_dir(tests_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ts") && !path.file_name().unwrap().to_str().unwrap().contains(".d.ts") {
                    let rel_path = path.strip_prefix(&workspace_root).unwrap().to_str().unwrap().replace("\\", "/");
                    test_files.push(rel_path);
                }
            }
        }
    }

    if test_files.is_empty() {
        eprintln!("❌ No test files found.");
        return;
    }

    let mut all_reports: Vec<(String, Vec<(String, u64, u64)>)> = Vec::new();
    let mut overall_success = true;

    for test_file in test_files {
        println!("\n🔄 {} {}", "Running test:".cyan().bold(), test_file.white());

        // Re-seed processed_signatures to the current chain state before each test.
        // This ensures any late-arriving transactions from the previous test (or any
        // pre-existing history) are marked as already-seen and never attributed to
        // this test's profile data.
        for pid_str in &program_ids {
            if let Ok(pid) = Address::from_str(pid_str) {
                if let Ok(sigs) = client.get_signatures_for_address(&pid) {
                    let mut processed = processed_signatures.lock().unwrap();
                    for sig_info in sigs {
                        processed.insert(sig_info.signature);
                    }
                }
            }
        }

        // Clear the profile data — observer will now only capture NEW transactions
        // that arrive after the re-seed above.
        {
            let mut data = profile_data.lock().unwrap();
            data.clear();
        }

        // Extract base test script from toml
        let mut test_script = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("test =") {
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed.rfind('"') {
                        if start != end {
                            test_script = trimmed[start + 1..end].to_string();
                            test_script = test_script.replace("\\\"", "\"");
                        }
                    }
                }
            }
        }

        if test_script.is_empty() {
            eprintln!("❌ Error: No test script found in Naclac.toml under [scripts].");
            std::process::exit(1);
        }

        // Inject the specific test file
        if test_script.contains("\"tests/**/*.ts\"") {
            test_script = test_script.replace("\"tests/**/*.ts\"", &format!("\"{}\"", test_file));
        } else if test_script.contains("tests/**/*.ts") {
            test_script = test_script.replace("tests/**/*.ts", &test_file);
        } else {
            test_script = format!("{} \"{}\"", test_script, test_file);
        }

        let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
        let shell_flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };

        let mut child = std::process::Command::new(shell)
            .arg(shell_flag)
            .arg(&test_script)
            .current_dir(&workspace_root)
            .env("TS_NODE_TRANSPILE_ONLY", "1")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute test script");

        let status = child.wait().expect("Failed to wait on test execution");
        
        // Give the RPC enough time to confirm and index the final transactions for this test.
        // Devnet can be slow; 6s is a safe minimum.
        thread::sleep(Duration::from_secs(6));

        // Process data for this specific test
        let raw_data = profile_data.lock().unwrap();
        if !raw_data.is_empty() {
            let mut aggregated: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
            for (name, cu) in raw_data.iter() {
                let entry = aggregated.entry(name.clone()).or_insert((0, 0));
                entry.0 += 1; // count
                entry.1 += cu; // total_cu
            }

            let mut final_data: Vec<(String, u64, u64)> = aggregated
                .into_iter()
                .map(|(name, (count, total_cu))| (name, count, total_cu / count))
                .collect();

            // Sort by Avg CU ascending
            final_data.sort_by_key(|&(_, _, avg_cu)| avg_cu);
            all_reports.push((test_file.clone(), final_data));
        } else {
            println!("⚠️  No transactions were captured for {}", test_file);
        }

        if !status.success() {
            eprintln!("❌ Test {} failed.", test_file);
            overall_success = false;
            break; // Stop running subsequent tests if one fails
        }
    }
    
    // Shut down observer
    tests_finished.store(true, Ordering::SeqCst);
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

fn print_final_summary(data: &[(String, u64, u64)]) {
    println!("\n{}", "📊 NACLAC FINAL COMPUTE REPORT".bold().green());
    println!("{}", "────────────────────────────────────────────────────────────────────────────────".dimmed());
    println!(
        "{:<25} {:<10} {:<15} {:<15} {:<15}",
        "Instruction".bold(),
        "Calls".bold(),
        "Avg CU".bold(),
        "Est. Cost".bold(),
        "Visual".bold()
    );
    println!("{}", "────────────────────────────────────────────────────────────────────────────────".dimmed());

    for (name, count, avg_cu) in data {
        let cu_color = if *avg_cu > 180_000 {
            Color::Red
        } else if *avg_cu > 100_000 {
            Color::Yellow
        } else if *avg_cu > 50_000 {
            Color::Cyan
        } else {
            Color::Green
        };

        let cost_sol = *avg_cu as f64 * 0.000000001;
        
        let bar_len = (*avg_cu as f64 / 200_000.0 * 20.0).min(20.0) as usize;
        let bar = "█".repeat(bar_len);
        let empty = "░".repeat(20 - bar_len);

        println!(
            "{:<25} {:<10} {:<15} {:<15} {}{}",
            name.yellow(),
            count.to_string().cyan(),
            format!("{} CU", avg_cu).color(cu_color),
            format!("{:.7} SOL", cost_sol).dimmed(),
            bar.color(cu_color),
            empty.dimmed()
        );
    }
    println!("{}", "────────────────────────────────────────────────────────────────────────────────".dimmed());
}
