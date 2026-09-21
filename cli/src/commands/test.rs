use std::fs;
use std::process::Command;

pub fn execute(
    target_file: Option<&str>,
    program: Option<&str>,
    rust: bool,
    node: bool,
    all: bool,
) {
    let run_node = node && !rust;

    if all {
        crate::commands::build::execute(None, Vec::new(), false);
        crate::commands::deploy::execute(None);
    }
    let current_dir = std::env::current_dir().unwrap();

    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        eprintln!(
            "❌ Error: Could not find Naclac.toml. Please run from within a Naclac workspace."
        );
        std::process::exit(1);
    };

    let toml_path = workspace_root.join("Naclac.toml");
    let toml_content = fs::read_to_string(&toml_path).unwrap();

    let mut test_script = String::new();
    let mut run_scripts = Vec::new();

    if run_node {
        // Parse test-node or test script from Naclac.toml safely
        for key in &["test-node", "test"] {
            let key_str = format!("{} =", key);
            for line in toml_content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with(&key_str) {
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
            if !test_script.is_empty() {
                break;
            }
        }

        if test_script.is_empty() {
            // Fallback default TypeScript/Node test script
            test_script =
                "yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\"".to_string();
        }

        // If target file specified, swap out/inject it
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
        // Rust testing
        // Parse test-rust script from Naclac.toml safely
        for key in &["test-rust", "test"] {
            let key_str = format!("{} =", key);
            let mut found_script = String::new();
            for line in toml_content.lines() {
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
            // Only use "test" if it contains cargo (so it's a Rust script)
            if !found_script.is_empty() && (key == &"test-rust" || found_script.contains("cargo")) {
                test_script = found_script;
                break;
            }
        }

        if test_script.is_empty() {
            test_script = "cargo test".to_string();
        }

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
                let test_target = after_test.split_whitespace().next().unwrap_or("");
                test_script = test_script.replace(test_target, &clean_f);
            } else {
                if let Some(idx) = test_script.find(" -- ") {
                    test_script.insert_str(idx, &format!(" --test {}", clean_f));
                } else {
                    test_script = format!("{} --test {}", test_script, clean_f);
                }
            }
        }

        // Build list of packages to test sequentially to avoid Cargo feature unification conflicts.
        let mut run_programs = Vec::new();
        if let Some(p) = program {
            run_programs.push(p.to_string());
        } else if let Ok(parsed) = toml::from_str::<toml::Value>(&toml_content) {
            let cluster = parsed
                .get("provider")
                .and_then(|p| p.get("cluster"))
                .and_then(|c| c.as_str())
                .unwrap_or("localnet");

            let mut found_table = None;
            if let Some(programs) = parsed.get("programs") {
                if let Some(tbl) = programs.get(cluster).and_then(|c| c.as_table()) {
                    found_table = Some(tbl);
                } else if let Some(tbl) = programs.get("devnet").and_then(|c| c.as_table()) {
                    found_table = Some(tbl);
                }
            }

            if let Some(tbl) = found_table {
                for (name, _) in tbl {
                    run_programs.push(name.clone());
                }
            }
        }

        if run_programs.is_empty() {
            run_scripts.push(test_script.clone());
        } else {
            for p in run_programs {
                let mut s = test_script.clone();
                if let Some(idx) = s.find(" -- ") {
                    s.insert_str(idx, &format!(" --package {}", p));
                } else {
                    s = format!("{} --package {}", s, p);
                }
                run_scripts.push(s);
            }
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

    let mut overall_success = true;

    if run_node {
        eprintln!("🧪 Running Naclac Test Suite...");
        eprintln!("   > {}", test_script);

        let mut command = Command::new(shell);
        command
            .arg(shell_flag)
            .arg(&test_script)
            .current_dir(&workspace_root)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        command.env("TS_NODE_TRANSPILE_ONLY", "1"); // Bypass slow TS checking

        let mut child = command.spawn().expect("Failed to execute test script");
        let status = child.wait().expect("Failed to wait on test execution");
        overall_success = status.success();
    } else {
        for script in &run_scripts {
            eprintln!("🧪 Running Naclac Test Suite...");
            eprintln!("   > {}", script);

            let mut command = Command::new(shell);
            command
                .arg(shell_flag)
                .arg(script)
                .current_dir(&workspace_root)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit());

            let mut child = command.spawn().expect("Failed to execute test script");
            let status = child.wait().expect("Failed to wait on test execution");
            if !status.success() {
                overall_success = false;
                break;
            }
        }
    }

    if overall_success {
        eprintln!("✅ All tests passed successfully!");
    } else {
        eprintln!("❌ Test suite failed.");
        std::process::exit(1);
    }
}
