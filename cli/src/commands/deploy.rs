use std::{fs, process::Command};

pub fn execute(program_id: Option<&str>) {
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

    let deploy_dir = naclac_client_gen::resolve_target_dir(&workspace_root).join("deploy");
    if !deploy_dir.exists() {
        eprintln!(
            "❌ Error: 'target/deploy' directory not found. Please run `naclac build` first."
        );
        std::process::exit(1);
    }

    // --- Parse Naclac.toml for Cluster and Wallet ---
    let toml_path = workspace_root.join("Naclac.toml");
    let toml_content = fs::read_to_string(&toml_path).unwrap();

    let mut cluster_setting = "localnet".to_string();
    let mut wallet_setting = "~/.config/solana/id.json".to_string();

    for line in toml_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("cluster =") {
            if let Some(val) = trimmed.split('"').nth(1) {
                cluster_setting = val.to_string();
            }
        }
        if trimmed.starts_with("wallet =") {
            if let Some(val) = trimmed.split('"').nth(1) {
                wallet_setting = val.to_string();
            }
        }
    }

    // Map cluster names to actual RPC URLs
    let cluster_url = match cluster_setting.as_str() {
        "localnet" => "http://127.0.0.1:8899",
        "devnet" => "https://api.devnet.solana.com",
        "testnet" => "https://api.testnet.solana.com",
        "mainnet" => "https://api.mainnet-beta.solana.com",
        custom => custom, // If they pasted a QuickNode/Helius URL, use it directly!
    };

    // Resolve the '~' in the wallet path to the actual Home Directory
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let expanded_wallet = wallet_setting.replace("~", &home_dir);

    let mut so_files = Vec::new();
    for entry in fs::read_dir(&deploy_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().unwrap_or_default() == "so" {
            let program_name = path.file_stem().unwrap().to_str().unwrap();
            let is_target = program_id.is_none_or(|tgt| program_name == tgt);
            if is_target {
                so_files.push(path);
            }
        }
    }

    if so_files.is_empty() {
        eprintln!("❌ Error: No compiled .so programs found. Please run `naclac build` first.");
        std::process::exit(1);
    }

    // --- Anchor-Parity Console Output ---
    for so_file in so_files {
        let program_name = so_file.file_stem().unwrap().to_str().unwrap();

        println!("Deploying cluster: {}", cluster_url);
        println!("Upgrade authority: {}", expanded_wallet);
        println!("Deploying program \"{}\"...", program_name);
        println!("Program path: {}\n", so_file.display());

        let output = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(&so_file)
            .arg("--url")
            .arg(cluster_url)
            .arg("--keypair")
            .arg(&expanded_wallet)
            .output()
            .expect("Failed to execute solana program deploy");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            for line in stdout.lines() {
                let trimmed = line.trim();
                if !is_ephemeral_recovery_line(trimmed) && !trimmed.is_empty() {
                    println!("{}", trimmed);
                }
            }
            for line in stderr.lines() {
                let trimmed = line.trim();
                if !is_ephemeral_recovery_line(trimmed) && !trimmed.is_empty() {
                    eprintln!("{}", trimmed);
                }
            }
            println!("\n✅ Successfully deployed '{}'!", program_name);
        } else {
            if !stdout.is_empty() {
                print!("{}", stdout);
            }
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
            eprintln!(
                "\n❌ Failed to deploy '{}'. Run `naclac deploy` again to resume the deployment.",
                program_name
            );
            std::process::exit(1);
        }
    }
}

fn is_ephemeral_recovery_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("====") {
        return true;
    }
    if trimmed.contains("Recover the intermediate account")
        || trimmed.contains("solana-keygen recover")
        || trimmed.contains("following 12-word seed phrase")
        || trimmed.contains("To resume a deploy")
        || trimmed.contains("[BUFFER_SIGNER]")
        || trimmed.contains("solana program write-buffer")
        || trimmed.contains("Or to recover the account's lamports")
        || trimmed.contains("solana program close")
    {
        return true;
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if (words.len() == 12 || words.len() == 24)
        && words
            .iter()
            .all(|w| w.chars().all(|c| c.is_ascii_lowercase()))
    {
        return true;
    }
    false
}
