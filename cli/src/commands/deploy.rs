//! `naclac deploy`: deploys each program of `[programs.<cluster>]` with
//! `solana program deploy` and then uploads its IDL, unless `--no-idl`.

use {
    crate::{
        commands::{idl, keypair, programs, workspace},
        ui,
    },
    solana_signer::Signer,
    std::{io::ErrorKind, process::Command, time::Instant},
};

/// Deploys each program in `requested` (every program of
/// `[programs.<cluster>]` when empty, `cluster` being `[provider] cluster`) to
/// that cluster, signing with `[provider] wallet`, then uploads or updates its
/// IDL unless `no_idl`, leaving an IDL that is already current alone unless
/// `force_idl`. A failed deploy stops the command; a failed IDL upload is
/// reported, the remaining programs are still deployed, and the command exits
/// with status 1.
pub fn execute(requested: &[String], no_idl: bool, force_idl: bool) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let cluster_url = workspace::rpc_url(&cluster);
    let wallet = workspace::wallet(&config);
    let wallet_address = keypair::read(&wallet)
        .unwrap_or_else(|e| ui::error(e))
        .pubkey()
        .to_string();

    let on_cluster = workspace::on_cluster(&config);
    let names: Vec<String> = on_cluster.iter().map(|(name, _)| name.clone()).collect();
    let selected = programs::select(&names, requested);

    let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);
    let mut to_deploy = Vec::new();
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
        let already_deployed =
            idl::program_is_deployed(&cluster_url, address).unwrap_or_else(|e| ui::error(e));
        to_deploy.push((name, address, so_file, already_deployed));
    }

    let started = Instant::now();
    ui::header("deploy", &cluster);
    ui::kv(
        "wallet",
        format!("{} ({})", ui::short(&wallet_address), wallet.display()),
    );
    ui::kv("rpc", &cluster_url);

    let mut idls_uploaded = 0usize;
    let mut idls_failed = 0usize;
    let mut deployed = 0usize;
    let mut upgraded = 0usize;
    for (name, address, so_file, already_deployed) in &to_deploy {
        ui::section(name, ui::short(address));
        let (doing, done, verb) = if *already_deployed {
            ("Upgrading", "program upgraded", "upgrade")
        } else {
            ("Deploying", "program deployed", "deploy")
        };
        let step = ui::Step::start(format!("{doing} the program"));
        let output = match Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(so_file)
            .arg("--url")
            .arg(&cluster_url)
            .arg("--keypair")
            .arg(&wallet)
            .output()
        {
            Ok(output) => output,
            Err(e) if e.kind() == ErrorKind::NotFound => step.fail(
                "the `solana` CLI is not installed or not on PATH; `naclac deploy` runs \
                 `solana program deploy`.",
            ),
            Err(e) => step.fail(format!("could not run `solana program deploy`: {e}")),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            step.failed(format!("'{name}' failed to {verb}"));
            for line in stdout.lines().chain(stderr.lines()) {
                ui::info(line);
            }
            ui::info("Run `naclac deploy` again to resume the deployment.");
            std::process::exit(1);
        }
        step.done(done);
        if *already_deployed {
            upgraded += 1;
        } else {
            deployed += 1;
        }
        if ui::show_output() {
            for line in stdout.lines().chain(stderr.lines()) {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !is_ephemeral_recovery_line(trimmed) {
                    ui::info(format!("  {trimmed}"));
                }
            }
        }

        if no_idl {
            continue;
        }
        match idl::execute_upload(address, force_idl) {
            Ok(()) => idls_uploaded += 1,
            Err(e) => {
                idls_failed += 1;
                ui::error_line(format!("IDL upload failed: {e}"));
                ui::info(format!(
                    "The program is deployed. Run `naclac idl upload {name}` to retry."
                ));
            }
        }
    }
    ui::end_section();

    if idls_failed > 0 {
        ui::error(format!(
            "{idls_failed} of {} IDL uploads failed.",
            to_deploy.len()
        ));
    }
    let plural = |count: usize| if count == 1 { "" } else { "s" };
    let mut programs_text = Vec::new();
    if deployed > 0 {
        programs_text.push(format!("Deployed {deployed} program{}", plural(deployed)));
    }
    if upgraded > 0 {
        programs_text.push(format!("Upgraded {upgraded} program{}", plural(upgraded)));
    }
    let programs_text = programs_text.join(" · ");
    let idls_text = if no_idl {
        String::new()
    } else {
        format!(
            " · {idls_uploaded} IDL{}",
            if idls_uploaded == 1 { "" } else { "s" }
        )
    };
    ui::milestone(format!(
        "{programs_text}{idls_text} · {}",
        ui::format_duration(started.elapsed())
    ));
}

/// Whether `line` of `solana program deploy`'s output is the recovery
/// information for its intermediate buffer (a seed phrase and the commands to
/// resume or close it), which a successful run has no use for.
pub fn is_ephemeral_recovery_line(line: &str) -> bool {
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
