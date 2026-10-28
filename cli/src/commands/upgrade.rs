use {
    crate::{
        commands::{programs, workspace},
        ui,
    },
    solana_address::Address,
    std::{process::Command, str::FromStr},
};

pub fn execute(
    program_id_str: &String,
    name: Option<&str>,
    filepath: Option<&str>,
    buffer: Option<&str>,
) {
    let program_id = Address::from_str(program_id_str).unwrap_or_else(|_| {
        ui::error(format!(
            "`{program_id_str}` is not a valid program address."
        ))
    });

    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let wallet_path = workspace::wallet(&config);
    let rpc_url = workspace::rpc_url(&cluster);

    match name {
        Some(name) => programs::require_synced_ids(&workspace_root, name, program_id_str),
        None => ui::warn(format!(
            "{program_id} is not listed in `[programs.{cluster}]`; its keypair, lib.rs and \
             Naclac.toml addresses were not checked."
        )),
    }

    ui::info(format!(
        "Upgrading {} on {} via {}",
        program_id,
        cluster,
        wallet_path.display()
    ));

    if let Some(buf) = buffer {
        let mut deploy_cmd = Command::new("solana")
            .arg("program")
            .arg("upgrade")
            .arg(buf)
            .arg(program_id_str)
            .arg("--url")
            .arg(&rpc_url)
            .arg("--keypair")
            .arg(&wallet_path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute native Solana program buffer upgrade.");

        let deploy_status = deploy_cmd
            .wait()
            .expect("Failed to finalize buffer migration.");

        if deploy_status.success() {
            ui::success("Upgrade rolled up from buffer.");
        } else {
            ui::error("Upgrade deployment failed.");
        }
    } else {
        let deploy_dir = naclac_client_gen::deploy_dir(&workspace_root);

        let so_file_path = match (filepath, name) {
            (Some(path), _) => path.to_string(),
            (None, Some(name)) => {
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
                so_file.to_string_lossy().into_owned()
            }
            (None, None) => ui::error(format!(
                "{program_id} is not listed in `[programs.{cluster}]`, so its compiled program \
                 cannot be found. Pass it with --filepath."
            )),
        };

        let mut deploy_cmd = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(&so_file_path)
            .arg("--program-id")
            .arg(program_id_str)
            .arg("--url")
            .arg(&rpc_url)
            .arg("--keypair")
            .arg(&wallet_path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to execute native Solana program deployer.");

        let deploy_status = deploy_cmd
            .wait()
            .expect("Failed to finalize buffer migration.");

        if deploy_status.success() {
            ui::success("Upgrade rolled up.");
        } else {
            ui::error("Upgrade deployment failed.");
        }
    }

    let mut buffer_cmd = Command::new("solana")
        .arg("program")
        .arg("close")
        .arg("--buffers")
        .arg("--url")
        .arg(&rpc_url)
        .arg("--keypair")
        .arg(&wallet_path)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("Failed to execute buffer clean-up sequence.");

    let buffer_status = buffer_cmd.wait().unwrap();

    if buffer_status.success() {
        ui::success("Stale buffers closed, rent refunded.");
    } else {
        ui::warn("Buffer cleanup didn't fully complete — check balances/RPC config.");
    }
}
