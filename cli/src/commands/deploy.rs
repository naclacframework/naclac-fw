use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    std::process::Command,
};

/// Deploys each program in `requested` (every program of
/// `[programs.<cluster>]` when empty, `cluster` being `[provider] cluster`) to
/// that cluster, signing with `[provider] wallet`.
pub fn execute(requested: &[String]) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster_url = workspace::rpc_url(&workspace::cluster(&config));
    let wallet = workspace::wallet(&config);

    let on_cluster = workspace::on_cluster(&config);
    let names: Vec<String> = on_cluster.iter().map(|(name, _)| name.clone()).collect();
    let selected = programs::select(&names, requested);

    let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);
    let mut so_files = Vec::new();
    for (name, address) in on_cluster
        .iter()
        .filter(|(name, _)| selected.contains(name))
    {
        programs::require_synced_ids(&workspace_root, name, address);
        let so_file = deploy_dir.join(format!("{name}.so"));
        let built = so_file
            .try_exists()
            .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", so_file.display())));
        if !built {
            ui::error(format!(
                "No compiled program for '{name}' at {}. Run `naclac build` first.",
                so_file.display()
            ));
        }
        so_files.push((name, so_file));
    }

    // --- Anchor-Parity Console Output ---
    for (program_name, so_file) in so_files {
        println!("Deploying cluster: {}", cluster_url);
        println!("Upgrade authority: {}", wallet.display());
        println!("Deploying program \"{}\"...", program_name);
        println!("Program path: {}\n", so_file.display());

        let output = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(&so_file)
            .arg("--url")
            .arg(&cluster_url)
            .arg("--keypair")
            .arg(&wallet)
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
