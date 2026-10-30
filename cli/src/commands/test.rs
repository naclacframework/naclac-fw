use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    std::process::Command,
};

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

    if run_node {
        if !requested.is_empty() {
            ui::error("`-p` selects programs for the Rust tests; the Node tests do not take it.");
        }
        test_script = workspace::node_test_script(&config);

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
                let test_target = after_test.split_whitespace().next().unwrap_or("");
                test_script = test_script.replace(test_target, &clean_f);
            } else if let Some(idx) = test_script.find(" -- ") {
                test_script.insert_str(idx, &format!(" --test {}", clean_f));
            } else {
                test_script = format!("{} --test {}", test_script, clean_f);
            }
        }

        // Each program is tested in its own `cargo test --package`, never all
        // together: programs with different account layouts enable
        // naclac-core features that cannot be combined in one build.
        let listed = programs::listed(&config);
        for p in programs::select(&listed, requested) {
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
