//! `naclac upgrade`: upgrades already-deployed programs from their compiled
//! `.so`, a file, or a staged buffer, and then updates each program's IDL
//! unless `--no-idl`.

use {
    crate::{
        commands::{deploy, idl, keypair, programs, workspace},
        ui,
    },
    solana_address::Address,
    solana_signer::Signer,
    std::{io::ErrorKind, path::PathBuf, process::Command, str::FromStr, time::Instant},
};

/// Where an upgrade's new program bytes come from.
enum Source {
    Buffer(String),
    File(PathBuf),
}

struct Plan<'a> {
    address: &'a str,
    name: Option<&'a str>,
    source: Source,
}

/// Upgrades each `(address, name)` of `targets` (`name` being its entry in
/// `[programs.<cluster>]`, if it has one) from `buffer` when given, else from
/// `filepath`, else from the compiled `.so` of its name, signing with
/// `[provider] wallet`. Every target is checked before the first upgrade
/// starts. A failed upgrade stops the command and leaves its buffer for a
/// resume; no other buffer of the wallet is ever touched. Afterwards each
/// program's IDL is updated unless `no_idl`, and left alone when already
/// current unless `force_idl`; a failed IDL update is reported, the remaining
/// programs are still upgraded, and the command exits with status 1.
pub fn execute(
    targets: &[(String, Option<String>)],
    filepath: Option<&str>,
    buffer: Option<&str>,
    no_idl: bool,
    force_idl: bool,
) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let wallet_path = workspace::wallet(&config);
    let rpc_url = workspace::rpc_url(&cluster);
    let wallet_address = keypair::read(&wallet_path)
        .unwrap_or_else(|e| ui::error(e))
        .pubkey()
        .to_string();

    let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);
    let mut plans = Vec::new();
    for (address, name) in targets {
        Address::from_str(address)
            .unwrap_or_else(|_| ui::error(format!("`{address}` is not a valid program address.")));
        if !idl::program_is_deployed(&rpc_url, address).unwrap_or_else(|e| ui::error(e)) {
            ui::error(format!(
                "{address} is not deployed on {cluster}; use `naclac deploy` for a first \
                 deployment."
            ));
        }
        if let Some(name) = name {
            programs::require_synced_ids(&workspace_root, name, address);
        }
        let source = match (buffer, filepath, name) {
            (Some(buffer), _, _) => Source::Buffer(buffer.to_string()),
            (None, Some(path), _) => Source::File(PathBuf::from(path)),
            (None, None, Some(name)) => {
                let so_file = deploy_dir.join(format!("{name}.so"));
                let built = so_file.try_exists().unwrap_or_else(|e| {
                    ui::error(format!("Cannot check for {}: {e}", so_file.display()))
                });
                if !built {
                    ui::error(format!(
                        "No compiled program for '{name}' at {}. Run `naclac build` first.",
                        so_file.display()
                    ));
                }
                Source::File(so_file)
            }
            (None, None, None) => ui::error(format!(
                "{address} is not listed in `[programs.{cluster}]`, so its compiled program \
                 cannot be found. Pass it with --filepath."
            )),
        };
        plans.push(Plan {
            address,
            name: name.as_deref(),
            source,
        });
    }

    let started = Instant::now();
    ui::header("upgrade", &cluster);
    ui::kv(
        "wallet",
        format!("{} ({})", ui::short(&wallet_address), wallet_path.display()),
    );
    ui::kv("rpc", &rpc_url);

    let mut idls_updated = 0usize;
    let mut idls_failed = 0usize;
    for plan in &plans {
        ui::section(
            plan.name
                .map_or_else(|| ui::short(plan.address), str::to_string),
            ui::short(plan.address),
        );
        if plan.name.is_none() {
            ui::warn(format!(
                "{} is not listed in `[programs.{cluster}]`; its keypair, lib.rs and \
                 Naclac.toml addresses were not checked.",
                plan.address
            ));
        }

        let step = ui::Step::start("Upgrading the program");
        let mut command = Command::new("solana");
        match &plan.source {
            Source::Buffer(buffer) => command
                .arg("program")
                .arg("upgrade")
                .arg(buffer)
                .arg(plan.address),
            Source::File(path) => command
                .arg("program")
                .arg("deploy")
                .arg(path)
                .arg("--program-id")
                .arg(plan.address),
        };
        command
            .arg("--url")
            .arg(&rpc_url)
            .arg("--keypair")
            .arg(&wallet_path);
        let output = match command.output() {
            Ok(output) => output,
            Err(e) if e.kind() == ErrorKind::NotFound => step.fail(
                "the `solana` CLI is not installed or not on PATH; `naclac upgrade` runs \
                 `solana program deploy` and `solana program upgrade`.",
            ),
            Err(e) => step.fail(format!("could not run the `solana` CLI: {e}")),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            step.failed("the upgrade failed");
            for line in stdout.lines().chain(stderr.lines()) {
                ui::info(line);
            }
            ui::info("Run the same `naclac upgrade` command again to resume.");
            std::process::exit(1);
        }
        step.done("program upgraded");
        if ui::show_output() {
            for line in stdout.lines().chain(stderr.lines()) {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !deploy::is_ephemeral_recovery_line(trimmed) {
                    ui::info(format!("  {trimmed}"));
                }
            }
        }

        if no_idl {
            continue;
        }
        if plan.name.is_none() {
            ui::warn("No IDL updated: the program is not listed, so no compiled IDL is known.");
            continue;
        }
        match idl::execute_upload(plan.address, force_idl) {
            Ok(()) => idls_updated += 1,
            Err(e) => {
                idls_failed += 1;
                ui::error_line(format!("IDL update failed: {e}"));
                ui::info(format!(
                    "The program is upgraded. Run `naclac idl upload {}` to retry.",
                    plan.name.unwrap_or(plan.address)
                ));
            }
        }
    }
    ui::end_section();

    if idls_failed > 0 {
        ui::error(format!(
            "{idls_failed} of {} IDL updates failed.",
            plans.len()
        ));
    }
    let programs_text = format!(
        "{} program{}",
        plans.len(),
        if plans.len() == 1 { "" } else { "s" }
    );
    let idls_text = if no_idl {
        String::new()
    } else {
        format!(
            " · {idls_updated} IDL{}",
            if idls_updated == 1 { "" } else { "s" }
        )
    };
    ui::milestone(format!(
        "Upgraded {programs_text}{idls_text} · {}",
        ui::format_duration(started.elapsed())
    ));
}
