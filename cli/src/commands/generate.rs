use std::fs;

pub fn execute(program_id: Option<&str>) {
    // 1. Resolve workspace root
    let current_dir = std::env::current_dir().unwrap();
    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        eprintln!("❌ Error: Could not find Naclac.toml.");
        std::process::exit(1);
    };

    let target_idl_dir = workspace_root.join("target/idl");
    if !target_idl_dir.exists() {
        eprintln!("❌ Error: target/idl does not exist. Run naclac build first.");
        std::process::exit(1);
    }

    let mut target_json_paths = Vec::new();

    if let Some(pid) = program_id {
        for entry in fs::read_dir(&target_idl_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().unwrap_or_default() == "json" {
                let content = fs::read_to_string(&path).unwrap();
                // Just a cheap parse to see if it matches pid
                let idl: serde_json::Value = serde_json::from_str(&content).unwrap();
                if content.contains(pid) {
                    let program_name = idl
                        .get("metadata")
                        .and_then(|m| m.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
                        .to_string();
                    target_json_paths.push((path, program_name));
                    break;
                }
            }
        }
    } else {
        for entry in fs::read_dir(&target_idl_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().unwrap_or_default() == "json" {
                let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
                target_json_paths.push((path, file_stem));
            }
        }
    }

    if target_json_paths.is_empty() {
        eprintln!("❌ Error: No IDL found. Run naclac build first.");
        std::process::exit(1);
    }

    for (json_path, program_name) in target_json_paths {
        let content = fs::read_to_string(&json_path).unwrap();

        let program_dir = workspace_root.join("programs").join(&program_name);
        let mut is_zero_copy = false;
        if program_dir.exists() {
            let cargo_toml_path = program_dir.join("Cargo.toml");
            let mut use_pinocchio = false;
            let mut use_borsh = false;
            if cargo_toml_path.exists() {
                if let Ok(cargo_content) = fs::read_to_string(&cargo_toml_path) {
                    if let Ok(parsed) = toml::from_str::<toml::Value>(&cargo_content) {
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
                        }
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
            is_zero_copy = use_pinocchio || !use_borsh;
        }

        if is_zero_copy {
            let marker_path = workspace_root.join(format!("target/.{}-zero-copy", program_name));
            let _ = fs::write(&marker_path, "");
        }

        let res = naclac_client_gen::generate_sdk(&content, &program_name, &workspace_root);

        if is_zero_copy {
            let marker_path = workspace_root.join(format!("target/.{}-zero-copy", program_name));
            let _ = fs::remove_file(marker_path);
        }

        if let Err(e) = res {
            eprintln!("❌ Error Generating SDK for '{}': {}", program_name, e);
        }
    }
}
