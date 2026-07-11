use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

pub fn execute(program_id: Option<&str>, features: Vec<String>) {
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

    let programs_dir = workspace_root.join("programs");
    let target_deploy_dir = workspace_root.join("target/deploy");
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
        eprintln!("❌ Error: No valid Naclac programs found in workspace.");
        std::process::exit(1);
    };

    eprintln!("🔍 Checking Program ID sync status...");
    for program_dir in &program_dirs {
        let program_name = program_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let keypair_path = target_deploy_dir.join(format!("{}-keypair.json", program_name));

        if !keypair_path.exists() {
            eprintln!(
                "   🔑 Keypair missing for '{}'. Auto-generating...",
                program_name
            );
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
                        eprintln!(
                            "   🔄 Auto-Syncing lib.rs for '{}' to {}",
                            program_name, actual_address
                        );
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
                        eprintln!("   🔄 Auto-Syncing Naclac.toml for '{}'...", program_name);
                        toml_code
                            .replace_range(addr_start..addr_start + end_offset, &actual_address);
                        fs::write(&toml_path, toml_code).unwrap();
                    }
                }
            }
        }
    }

    eprintln!("🔨 Compiling Native SBF...");

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
    // once `generate::execute` runs later in this loop.
    for prog_dir in &programs_to_build {
        let pname = prog_dir.file_name().unwrap().to_str().unwrap();
        let pname_kebab = pname.replace('_', "-");
        let rust_client_dir = workspace_root.join("clients/rust").join(pname);
        if !rust_client_dir.join("Cargo.toml").exists() {
            eprintln!(
                "   📦 Generated Rust client missing — creating stub for '{}'...",
                pname
            );
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
                eprintln!(
                    "❌ Error: Failed to create directory {:?}: {}\n\
                     This often occurs on Windows/WSL when a directory was recently deleted but remains\n\
                     locked in a 'delete pending' state by a background process (e.g. VS Code, rust-analyzer, or terminal).\n\
                     Please close any tools accessing the clients folder and try again.",
                    src_dir, e
                );
                std::process::exit(1);
            }
            // Feature set here must stay in sync with the real generator's template
            // in naclac-client-gen/src/rust/mod.rs (generate_rust_sdk) — that's what
            // overwrites this stub once `naclac generate` runs. The real generator
            // always declares `zero_copy`/`pinocchio`/`borsh` unconditionally (only
            // `default` varies by mode), so the stub must too, or any program's
            // Cargo.toml that requests e.g. `features = ["borsh"]` on its generated
            // client dev-dependency fails `cargo metadata` during this bootstrap
            // window, before the real client ever gets a chance to replace the stub.
            let stub_cargo = format!(
                "[package]\nname = \"{pname_kebab}-client\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\ndefault = [\"offchain\"]\noffchain = [\"dep:naclac-client\"]\ncpi = [\"dep:naclac-lang\"]\nzero_copy = [\"naclac-lang/solana\"]\npinocchio = [\"naclac-lang/pinocchio\"]\nborsh = [\"naclac-lang/borsh\", \"zero_copy\"]\n\n[dependencies]\nnaclac-client = {{ path = \"../../../../../naclac-client\", version = \"0.1.0\", optional = true }}\nnaclac-lang = {{ path = \"../../../../../naclac-lang\", version = \"0.1.0\", optional = true, default-features = false }}\n"
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

        // Detect if specific features are active for this program.
        let mut use_pinocchio = false;
        let mut use_idl_build = false;
        let mut use_borsh = false;
        if cargo_toml_path.exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                    if let Some(features) = parsed.get("features").and_then(|f| f.as_table()) {
                        if let Some(default_feats) =
                            features.get("default").and_then(|d| d.as_array())
                        {
                            use_pinocchio = default_feats
                                .iter()
                                .any(|val| val.as_str() == Some("pinocchio"));
                            use_borsh = default_feats
                                .iter()
                                .any(|val| val.as_str() == Some("borsh"));
                        }
                        if !use_pinocchio {
                            use_pinocchio = features.contains_key("pinocchio");
                        }
                        use_idl_build = features.contains_key("idl-build");
                    }
                    // `borsh` may also be requested directly on the naclac-lang dependency
                    // rather than declared as the program's own default feature.
                    if !use_borsh {
                        use_borsh = parsed
                            .get("dependencies")
                            .and_then(|d| d.get("naclac-lang"))
                            .and_then(|dep| dep.get("features"))
                            .and_then(|f| f.as_array())
                            .map(|arr| arr.iter().any(|v| v.as_str() == Some("borsh")))
                            .unwrap_or(false);
                    }
                }
            }
        }

        // is_zero_copy = true when the program is Pinocchio, or when it isn't
        // requesting the `borsh` feature at all — there is no third
        // representation for account data in this framework, so `!borsh` is
        // a complete signal, not a heuristic.
        let is_zero_copy = use_pinocchio || !use_borsh;

        let mut cmd_builder = portable_pty::CommandBuilder::new("cargo");
        cmd_builder.arg("build-sbf");
        cmd_builder.arg("--manifest-path");
        cmd_builder.arg(cargo_toml_path.to_str().unwrap());
        cmd_builder.env("CARGO_TERM_COLOR", "always");
        cmd_builder.cwd(&workspace_root);

        let mut all_features = features.clone();
        if use_pinocchio {
            eprintln!(
                "   ⚡ Pinocchio feature detected for '{}' — building optimized no_std binary...",
                prog_name
            );
            all_features.push("pinocchio".to_string());
            cmd_builder.arg("--no-default-features");
        } else {
            eprintln!(
                "   📦 Building standard Solana program for '{}'...",
                prog_name
            );
            if use_idl_build {
                eprintln!(
                    "   📜 IDL-Build feature detected — injecting on-chain IDL instructions..."
                );
                all_features.push("idl-build".to_string());
            }
        }

        if !all_features.is_empty() {
            cmd_builder.arg("--features");
            cmd_builder.arg(all_features.join(","));
        }

        // A real pseudo-terminal — not a plain pipe — so cargo's own TTY check
        // passes and it draws its normal colored "Building [====>] N/M: ..."
        // progress bar exactly like it would in a real terminal. A plain pipe
        // makes cargo *decide not to generate those bytes at all*, which no
        // amount of reading/forwarding on our end can work around.
        let (term_rows, term_cols) = {
            let (rows, cols) = dialoguer::console::Term::stdout().size();
            (
                if rows > 0 { rows } else { 24 },
                if cols > 0 { cols } else { 80 },
            )
        };

        let pty_system = portable_pty::native_pty_system();
        let pty_pair = pty_system
            .openpty(portable_pty::PtySize {
                rows: term_rows,
                cols: term_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to allocate a pseudo-terminal for cargo build-sbf");

        let mut child = pty_pair
            .slave
            .spawn_command(cmd_builder)
            .expect("Failed to execute cargo build-sbf");
        drop(pty_pair.slave);

        let mut pty_reader = pty_pair
            .master
            .try_clone_reader()
            .expect("Failed to read from cargo build-sbf's pseudo-terminal");
        let capture_thread = std::thread::spawn(move || {
            let mut captured = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &buf[..n];
                        let _ = std::io::stdout().write_all(chunk);
                        let _ = std::io::stdout().flush();
                        captured.extend_from_slice(chunk);
                    }
                    Err(_) => break,
                }
            }
            captured
        });

        let build_status = child.wait().expect("Failed to wait on cargo build-sbf");
        drop(pty_pair.master);
        let combined_bytes = capture_thread.join().unwrap();
        let combined = String::from_utf8_lossy(&combined_bytes).to_string();
        let stack_overflow_detected = combined
            .contains("overflows the maximum allowed frame space")
            || combined.contains("exceeded max offset");

        if !build_status.success() || stack_overflow_detected {
            if stack_overflow_detected {
                eprintln!(
                    "❌ SBF Compilation for '{}' produced a stack-frame overflow. \
                     This does not always fail cargo build-sbf's own exit code, so naclac is \
                     treating it as fatal — see the 'overflows the maximum allowed frame space' \
                     message above for which function to fix.",
                    prog_name
                );
            } else {
                eprintln!("❌ SBF Compilation failed for '{}'.", prog_name);
            }
            std::process::exit(1);
        }

        eprintln!("📄 Generating Naclac IDL & Types for '{}'...", prog_name);

        let keypair_path = target_deploy_dir.join(format!("{}-keypair.json", prog_name));
        let address_output = Command::new("solana-keygen")
            .arg("pubkey")
            .arg(&keypair_path)
            .output()
            .unwrap();
        let actual_address = String::from_utf8_lossy(&address_output.stdout)
            .trim()
            .to_string();

        let idl_json_pretty = match naclac_idl::generate_idl(
            build_dir,
            prog_name,
            &actual_address,
            env!("CARGO_PKG_VERSION"),
            is_zero_copy,
        ) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("❌ Failed to generate IDL: {}", e);
                std::process::exit(1);
            }
        };

        let target_idl_dir = workspace_root.join("target/idl");
        fs::create_dir_all(&target_idl_dir).unwrap();
        let idl_path = target_idl_dir.join(format!("{}.json", prog_name));
        fs::write(&idl_path, &idl_json_pretty).unwrap();
        eprintln!(
            "✅ IDL written to: {:?}",
            idl_path.canonicalize().unwrap_or(idl_path)
        );

        let target_types_dir = workspace_root.join("target/types");
        fs::create_dir_all(&target_types_dir).unwrap();

        if is_zero_copy {
            let marker_path = workspace_root.join(format!("target/.{}-zero-copy", prog_name));
            let _ = fs::write(&marker_path, "");
        }

        let ts_content = match naclac_client_gen::generate_ts(&idl_json_pretty) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("❌ Failed to generate TS Types: {}", e);
                if is_zero_copy {
                    let marker_path =
                        workspace_root.join(format!("target/.{}-zero-copy", prog_name));
                    let _ = fs::remove_file(marker_path);
                }
                std::process::exit(1);
            }
        };

        let ts_path = target_types_dir.join(format!("{}.ts", prog_name));
        fs::write(&ts_path, ts_content).unwrap();
        eprintln!(
            "✅ Types written to: {:?}",
            ts_path.canonicalize().unwrap_or(ts_path)
        );
        eprintln!("🔄 Auto-generating TypeScript SDK for '{}'...", prog_name);
        crate::commands::generate::execute(Some(prog_name));

        if is_zero_copy {
            let marker_path = workspace_root.join(format!("target/.{}-zero-copy", prog_name));
            let _ = fs::remove_file(marker_path);
        }
    }
}
