use crate::ui;
use colored::Colorize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn execute(program_id: Option<&str>, features: Vec<String>) {
    let build_start = std::time::Instant::now();
    let current_dir = std::env::current_dir().unwrap();

    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        ui::error("Could not find Naclac.toml — run this from within a Naclac workspace.")
    };

    let programs_dir = workspace_root.join("programs");
    let target_dir = naclac_client_gen::resolve_target_dir(&workspace_root);
    let target_deploy_dir = target_dir.join("deploy");
    fs::create_dir_all(&target_deploy_dir).unwrap();

    let program_dirs: Vec<PathBuf> = if programs_dir.exists() {
        fs::read_dir(&programs_dir)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let is_target = program_id.is_none_or(|tgt| path.file_name().unwrap() == tgt);
                if path.is_dir() && path.join("src/lib.rs").exists() && is_target {
                    Some(path)
                } else {
                    None
                }
            })
            .collect()
    } else {
        ui::error("No valid Naclac programs found in workspace.")
    };

    for program_dir in &program_dirs {
        let program_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let keypair_path = target_deploy_dir.join(format!("{}-keypair.json", program_name));

        if !keypair_path.exists() {
            ui::warn(format!(
                "Keypair missing for '{}' — generated",
                program_name
            ));
            Command::new("solana-keygen")
                .arg("new")
                .arg("--no-bip39-passphrase")
                .arg("-o")
                .arg(&keypair_path)
                .arg("--force")
                .output()
                .expect("Failed to generate keypair");
        }

        let address_output = Command::new("solana-keygen")
            .arg("pubkey")
            .arg(&keypair_path)
            .output()
            .expect("Failed to get address");
        let actual_address = String::from_utf8_lossy(&address_output.stdout)
            .trim()
            .to_string();

        let lib_path = program_dir.join("src/lib.rs");
        if lib_path.exists() {
            let mut lib_code = fs::read_to_string(&lib_path).unwrap();
            if let Some(start) = lib_code.find("declare_id!(\"") {
                let addr_start = start + 13;
                if let Some(end_offset) = lib_code[addr_start..].find("\")") {
                    let current_address = &lib_code[addr_start..addr_start + end_offset];
                    if current_address != actual_address {
                        ui::warn(format!("Synced program ID -> lib.rs ({})", program_name));
                        lib_code
                            .replace_range(addr_start..addr_start + end_offset, &actual_address);
                        fs::write(&lib_path, lib_code).unwrap();
                    }
                }
            }
        }

        let toml_path = workspace_root.join("Naclac.toml");
        if toml_path.exists() {
            let mut toml_code = fs::read_to_string(&toml_path).unwrap();
            let search_str = format!("{} = \"", program_name);
            if let Some(start) = toml_code.find(&search_str) {
                let addr_start = start + search_str.len();
                if let Some(end_offset) = toml_code[addr_start..].find("\"") {
                    let current_address = &toml_code[addr_start..addr_start + end_offset];
                    if current_address != actual_address {
                        ui::warn(format!(
                            "Synced program ID -> Naclac.toml ({})",
                            program_name
                        ));
                        toml_code
                            .replace_range(addr_start..addr_start + end_offset, &actual_address);
                        fs::write(&toml_path, toml_code).unwrap();
                    }
                }
            }
        }
    }

    // For each program, detect if pinocchio feature is enabled and build accordingly.
    // If running a workspace-wide build (no specific program), we check all programs.
    let programs_to_build: Vec<PathBuf> = if let Some(tgt) = program_id {
        program_dirs
            .iter()
            .filter(|p| p.file_name().unwrap().to_str().unwrap() == tgt)
            .cloned()
            .collect()
    } else {
        program_dirs.clone()
    };

    // ── Pre-build stub pass ────────────────────────────────────────────────────
    // cargo metadata (called by cargo build-sbf internally) resolves ALL workspace
    // members — and their [dev-dependencies] — in a single pass before compilation
    // starts. If ANY generated Rust client is missing, the ENTIRE workspace metadata
    // resolution fails.
    //
    // Solution: ensure every program has a minimal valid generated-client stub BEFORE
    // we invoke cargo build-sbf on any program. The real client overwrites the stub
    // once `generate::execute_client` runs later in this loop.
    for prog_dir in &programs_to_build {
        let pname = prog_dir.file_name().unwrap().to_str().unwrap();
        let pname_kebab = pname.replace('_', "-");
        let rust_client_dir = workspace_root.join("clients/rust").join(pname);
        if !rust_client_dir.join("Cargo.toml").exists() {
            let src_dir = rust_client_dir.join("src");
            let mut result = fs::create_dir_all(&src_dir);
            if result.is_err() {
                for _ in 1..=5 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    result = fs::create_dir_all(&src_dir);
                    if result.is_ok() {
                        break;
                    }
                }
            }
            if let Err(e) = result {
                ui::error(format!(
                    "Failed to create directory {:?}: {} — often a Windows/WSL 'delete pending' \
                     lock from VS Code/rust-analyzer/a terminal; close anything touching the \
                     clients folder and retry.",
                    src_dir, e
                ));
            }
            // Feature set here must stay in sync with the real generator's template
            // in naclac-client-gen/src/rust/mod.rs (generate_rust_sdk) — that's what
            // overwrites this stub once `naclac generate` runs. The real generator
            // always declares `zero_copy`/`pinocchio`/`borsh` unconditionally (only
            // `default` varies by mode), so the stub must too, or any program's
            // Cargo.toml that requests e.g. `features = ["borsh"]` on its generated
            // client dev-dependency fails `cargo metadata` during this bootstrap
            // window, before the real client ever gets a chance to replace the stub.
            let (naclac_lang_path, naclac_client_path) =
                naclac_client_gen::naclac_dep_path_fragments(&workspace_root, &rust_client_dir);
            let stub_cargo = format!(
                "[package]\nname = \"{pname_kebab}-client\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\ndefault = [\"offchain\"]\noffchain = [\"dep:naclac-client\"]\ncpi = [\"dep:naclac-lang\"]\nzero_copy = [\"naclac-lang/solana\"]\npinocchio = [\"naclac-lang/pinocchio\"]\nborsh = [\"naclac-lang/borsh\", \"zero_copy\"]\n\n[dependencies]\nnaclac-client = {{ version = \"0.1.0\", optional = true{naclac_client_path} }}\nnaclac-lang = {{ version = \"0.1.0\", optional = true, default-features = false{naclac_lang_path} }}\n"
            );
            fs::write(rust_client_dir.join("Cargo.toml"), &stub_cargo).unwrap();
            fs::write(
                rust_client_dir.join("src/lib.rs"),
                "// Stub — will be overwritten by naclac generate\n",
            )
            .unwrap();
        }
    }

    for build_dir in &programs_to_build {
        let prog_name = build_dir.file_name().unwrap().to_str().unwrap();
        let cargo_toml_path = build_dir.join("Cargo.toml");

        // Shared with `naclac generate idl` — see generate.rs, the single
        // implementation of "what does regenerating a program's IDL mean."
        let prog_features = crate::commands::generate::detect_program_features(build_dir);
        let use_pinocchio = prog_features.use_pinocchio;
        let is_zero_copy = prog_features.is_zero_copy;

        let mode_label = if use_pinocchio {
            "pinocchio"
        } else {
            "standard"
        };
        let mut all_features = features.clone();
        if use_pinocchio {
            all_features.push("pinocchio".to_string());
        }

        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf")
            .arg("--manifest-path")
            .arg(cargo_toml_path.to_str().unwrap())
            .env("CARGO_TERM_COLOR", "always")
            .current_dir(&workspace_root)
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped());
        if use_pinocchio {
            cmd.arg("--no-default-features");
        }
        if !all_features.is_empty() {
            cmd.arg("--features").arg(all_features.join(","));
        }

        let mut child = cmd.spawn().expect("Failed to execute cargo build-sbf");
        let stderr = child
            .stderr
            .take()
            .expect("Failed to capture cargo build-sbf stderr");

        // Piped (not a pseudo-terminal), so cargo skips its own cursor-redrawn
        // progress bar and just prints one plain "Compiling <crate>" line per
        // crate on stderr as they start — parsed below to drive our own
        // single-line spinner instead, since forwarding cargo's raw redraw
        // bytes while dropping lines from the stream desyncs its cursor math
        // (it moves the cursor up assuming every line it emitted is still on
        // screen).
        let step = ui::Step::start(format!("Compiling {} ({})...", prog_name, mode_label));
        let mut combined = String::new();
        let mut compiled = 0u32;
        let mut printed_output_header = false;
        let mut cargo_error_summary: Option<String> = None;
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            combined.push_str(&line);
            combined.push('\n');
            if let Some(crate_name) = cargo_compiling_crate_name(&line) {
                compiled += 1;
                step.set_message(format!(
                    "Compiling {} ({})... {} crate{} built ({})",
                    prog_name,
                    mode_label,
                    compiled,
                    if compiled == 1 { "" } else { "s" },
                    crate_name
                ));
            } else if !line.trim().is_empty() && !is_cargo_finished_line(&line) {
                if !printed_output_header {
                    step.print_above("── compiler output ──────────────────────".dimmed());
                    printed_output_header = true;
                }
                // cargo's own final tally line ("error: could not compile `X` ...
                // due to N previous errors" / "error: aborting due to N previous
                // errors") — the authoritative error count, straight from cargo
                // itself rather than naclac re-deriving one by pattern-matching
                // individual diagnostics.
                let stripped = strip_ansi(&line);
                let trimmed = stripped.trim();
                if trimmed.starts_with("error: could not compile")
                    || trimmed.starts_with("error: aborting due to")
                {
                    cargo_error_summary = Some(trimmed.to_string());
                }
                step.print_above(line);
            }
        }

        let build_status = child.wait().expect("Failed to wait on cargo build-sbf");
        let stack_overflow_detected = combined
            .contains("overflows the maximum allowed frame space")
            || combined.contains("exceeded max offset");

        if !build_status.success() || stack_overflow_detected {
            if stack_overflow_detected {
                step.fail(format!(
                    "'{}' hit a stack-frame overflow — see 'overflows the maximum allowed frame \
                     space' above for which function to fix.",
                    prog_name
                ));
            } else {
                match cargo_error_summary {
                    Some(summary) => {
                        step.fail(format!("Compile failed for '{}' — {}", prog_name, summary))
                    }
                    None => step.fail(format!("Compile failed for '{}'", prog_name)),
                }
            }
        }
        step.done(format!(
            "Compiled {} ({} crates built)",
            prog_name, compiled
        ));

        let idl_step = ui::Step::start(format!("Generating IDL & TS types ({})...", prog_name));

        // Shared with `naclac generate idl` — see generate.rs. Writes the
        // zero-copy marker (if any) with the real alloc-event names; left in
        // place deliberately so the client-gen call below sees it unmodified.
        let (idl_json_pretty, _alloc_event_names) =
            crate::commands::generate::generate_idl_for_program(
                &workspace_root,
                build_dir,
                prog_name,
                is_zero_copy,
            );

        let target_types_dir = target_dir.join("types");
        fs::create_dir_all(&target_types_dir).unwrap();

        let ts_content = match naclac_client_gen::generate_ts(&idl_json_pretty) {
            Ok(code) => code,
            Err(e) => {
                if is_zero_copy {
                    let marker_path = target_dir.join(format!(".{}-zero-copy", prog_name));
                    let _ = fs::remove_file(marker_path);
                }
                idl_step.fail(format!(
                    "Failed to generate TS types for '{}': {}",
                    prog_name, e
                ));
            }
        };

        let ts_path = target_types_dir.join(format!("{}.ts", prog_name));
        fs::write(&ts_path, ts_content).unwrap();
        idl_step.done(format!("IDL + TS types written ({})", prog_name));
        crate::commands::generate::execute_client(Some(prog_name), None);

        if is_zero_copy {
            let marker_path = target_dir.join(format!(".{}-zero-copy", prog_name));
            let _ = fs::remove_file(marker_path);
        }
    }

    ui::milestone(format!(
        "Build complete — {} program{}, {}",
        programs_to_build.len(),
        if programs_to_build.len() == 1 {
            ""
        } else {
            "s"
        },
        ui::format_duration(build_start.elapsed())
    ));
}

/// Removes ANSI CSI escape sequences (`\x1b[...<letter>`), the only kind
/// cargo emits, so line-content checks on a captured line see the same text
/// a human reads.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Extracts the crate name from one of cargo's own per-crate
/// "Compiling <name> v<version> (<path>)" stderr lines, or `None` for any
/// other line (warnings, errors, the final "Finished" line, etc.).
fn cargo_compiling_crate_name(raw: &str) -> Option<String> {
    let line = strip_ansi(raw);
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("Compiling ")?;
    let v_pos = rest.find(" v")?;
    let after_v = &rest[v_pos + 2..];
    if after_v.starts_with(|c: char| c.is_ascii_digit())
        && after_v.contains('(')
        && trimmed.ends_with(')')
    {
        Some(rest[..v_pos].to_string())
    } else {
        None
    }
}

/// True for cargo's own final "Finished `<profile>` profile [...] target(s)
/// in <N>s" line — dropped because naclac's own timed `✓ <elapsed> Compiled
/// ...` result line (see [`ui::Step::done`]) already reports the same thing.
fn is_cargo_finished_line(raw: &str) -> bool {
    strip_ansi(raw).trim().starts_with("Finished `")
}
