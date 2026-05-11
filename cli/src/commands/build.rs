use std::fs;
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
                let is_target = program_id.map_or(true, |tgt| path.file_name().unwrap() == tgt);
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

        let pubkey_output = Command::new("solana-keygen")
            .arg("pubkey")
            .arg(&keypair_path)
            .output()
            .expect("Failed to get pubkey");
        let actual_pubkey = String::from_utf8_lossy(&pubkey_output.stdout)
            .trim()
            .to_string();

        let lib_path = program_dir.join("src/lib.rs");
        if lib_path.exists() {
            let mut lib_code = fs::read_to_string(&lib_path).unwrap();
            if let Some(start) = lib_code.find("declare_id!(\"") {
                let addr_start = start + 13;
                if let Some(end_offset) = lib_code[addr_start..].find("\")") {
                    let current_pubkey = &lib_code[addr_start..addr_start + end_offset];
                    if current_pubkey != actual_pubkey {
                        eprintln!(
                            "   🔄 Auto-Syncing lib.rs for '{}' to {}",
                            program_name, actual_pubkey
                        );
                        lib_code.replace_range(addr_start..addr_start + end_offset, &actual_pubkey);
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
                    let current_pubkey = &toml_code[addr_start..addr_start + end_offset];
                    if current_pubkey != actual_pubkey {
                        eprintln!("   🔄 Auto-Syncing Naclac.toml for '{}'...", program_name);
                        toml_code
                            .replace_range(addr_start..addr_start + end_offset, &actual_pubkey);
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
        program_dirs.iter()
            .filter(|p| p.file_name().unwrap().to_str().unwrap() == tgt)
            .cloned()
            .collect()
    } else {
        program_dirs.clone()
    };

    for build_dir in &programs_to_build {
        let prog_name = build_dir.file_name().unwrap().to_str().unwrap();
        let cargo_toml_path = build_dir.join("Cargo.toml");

        // Detect if specific features are active for this program.
        let mut active_features = Vec::new();
        if cargo_toml_path.exists() {
            let content = fs::read_to_string(&cargo_toml_path).unwrap_or_default();
            let mut in_features = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("[features]") {
                    in_features = true;
                    continue;
                }
                if trimmed.starts_with("[") {
                    in_features = false;
                }
                
                if in_features {
                    if trimmed.starts_with("idl-build") {
                        active_features.push("idl-build");
                    }
                }

                // Detect Zero-Copy intent (Pinocchio, Zero-Copy, or No-Std)
                if trimmed.starts_with("pinocchio") || 
                   trimmed.starts_with("zero-copy") ||
                   trimmed.starts_with("no-std") ||
                   (trimmed.starts_with("default") && (trimmed.contains("pinocchio") || trimmed.contains("zero-copy"))) {
                    active_features.push("zero-copy-mode");
                }
                
                if trimmed.starts_with("pinocchio") {
                    active_features.push("pinocchio");
                }
            }
        }

        let use_pinocchio = active_features.contains(&"pinocchio");
        let is_zero_copy = active_features.contains(&"zero-copy-mode");
        let use_idl_build = active_features.contains(&"idl-build");

        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf")
            .arg("--manifest-path")
            .arg(cargo_toml_path.to_str().unwrap())
            .current_dir(&workspace_root);

        let mut all_features = features.clone();
        if use_pinocchio {
            eprintln!("   ⚡ Pinocchio feature detected for '{}' — building optimized no_std binary...", prog_name);
            all_features.push("pinocchio".to_string());
            cmd.arg("--no-default-features");
        } else {
            eprintln!("   📦 Building standard Solana program for '{}'...", prog_name);
            if use_idl_build {
                eprintln!("   📜 IDL-Build feature detected — injecting on-chain IDL instructions...");
                all_features.push("idl-build".to_string());
            }
        }

        if !all_features.is_empty() {
            cmd.arg("--features").arg(all_features.join(","));
        }

        let mut child = cmd
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute cargo build-sbf");

        let build_status = child.wait().expect("Failed to wait on cargo build-sbf");

        if !build_status.success() {
            eprintln!("❌ SBF Compilation failed for '{}'.", prog_name);
            std::process::exit(1);
        }

        eprintln!("📄 Generating Naclac IDL & Types for '{}'...", prog_name);

        let keypair_path = target_deploy_dir.join(format!("{}-keypair.json", prog_name));
        let pubkey_output = Command::new("solana-keygen")
            .arg("pubkey")
            .arg(&keypair_path)
            .output()
            .unwrap();
        let actual_pubkey = String::from_utf8_lossy(&pubkey_output.stdout)
            .trim()
            .to_string();

        let idl_json_pretty = match naclac_idl::generate_idl(
            &build_dir,
            &prog_name,
            &actual_pubkey,
            env!("CARGO_PKG_VERSION"),
            is_zero_copy
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

        let ts_content = match naclac_client_gen::generate_ts(&idl_json_pretty) {
             Ok(code) => code,
             Err(e) => {
                 eprintln!("❌ Failed to generate TS Types: {}", e);
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
        crate::commands::generate::execute(Some(&prog_name));
    }
}
