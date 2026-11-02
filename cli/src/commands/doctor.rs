use {crate::ui, std::process::Command};

pub fn execute() {
    ui::header("doctor", "environment");
    ui::set_indent(2);

    let checks: [(&str, &[&str]); 5] = [
        ("cargo-build-sbf", &["--version"]),
        ("rustc", &["--version"]),
        ("solana", &["--version"]),
        ("node", &["--version"]),
        ("yarn", &["--version"]),
    ];

    let mut passed = 0;
    let total = checks.len();

    for (cmd, args) in checks {
        match Command::new(cmd).args(args).output() {
            Ok(output) if output.status.success() => {
                passed += 1;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let version = stdout
                    .lines()
                    .next()
                    .unwrap_or("version not reported")
                    .trim();
                ui::success(format!("{cmd:<18} {version}"));
            }
            _ => {
                ui::error_line(format!("{cmd:<18} not found in PATH"));
            }
        }
    }

    ui::set_indent(0);
    if passed == total {
        ui::milestone(format!(
            "All {total} toolchain dependencies are operational"
        ));
    } else {
        ui::warn(format!(
            "{passed}/{total} dependencies operational — install missing tools to build and test"
        ));
    }
}
