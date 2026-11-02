use {
    crate::{commands::workspace, ui},
    sha2::{Digest, Sha256},
    std::{fs, process::Command, time::Instant},
};

/// Name the workspace's own Cargo.lock is moved to while the Docker build
/// runs.
const LOCK_BACKUP: &str = "Cargo.lock.naclac-bak";

/// Rebuilds the workspace in Docker and compares the program it produces for
/// `name` with the program deployed at `program_id_str` on `[provider]
/// cluster`. A mismatch or any failure is an error.
pub fn execute(program_id_str: &String, name: Option<&str>) {
    if bs58::decode(program_id_str).into_vec().is_err() {
        ui::error("Invalid base58 program ID.");
    }

    match Command::new("docker").arg("--version").output() {
        Ok(out) if out.status.success() => {}
        _ => ui::error(
            "Docker is not running or missing from PATH — naclac needs Docker for \
             deterministic verify builds. Install it (docs.docker.com/get-docker) and \
             make sure the daemon is running.",
        ),
    }

    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let rpc_url = workspace::rpc_url(&cluster);

    let Some(name) = name else {
        ui::error(format!(
            "{program_id_str} is not listed in `[programs.{cluster}]`, so its compiled \
             program cannot be found."
        ));
    };

    let start = Instant::now();
    ui::header("verify", name);
    ui::kv("program", format!("{name} ({})", ui::short(program_id_str)));
    ui::kv("cluster", &cluster);
    ui::kv("rpc", &rpc_url);
    ui::kv("image", "ellipsislabs/solana:latest");

    let parent_dir = workspace_root
        .parent()
        .unwrap_or_else(|| ui::error("The workspace root has no parent folder to mount."));
    let project_name = workspace_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| ui::error("The workspace folder's name is not valid text."));

    // The Docker image's cargo cannot read the host's lockfile format, so the
    // lockfile is moved aside for the build and moved back afterwards.
    let lock_path = workspace_root.join("Cargo.lock");
    let backup_path = workspace_root.join(LOCK_BACKUP);
    let has_lock = lock_path
        .try_exists()
        .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", lock_path.display())));
    if has_lock {
        let backup_exists = backup_path.try_exists().unwrap_or_else(|e| {
            ui::error(format!("Cannot check for {}: {e}", backup_path.display()))
        });
        if backup_exists {
            ui::error(format!(
                "{} exists from an earlier verify that did not finish; it is your original \
                 Cargo.lock. Move it back to {}, then run verify again.",
                backup_path.display(),
                lock_path.display()
            ));
        }
        fs::rename(&lock_path, &backup_path).unwrap_or_else(|e| {
            ui::error(format!(
                "Cannot move {} aside for the Docker build: {e}",
                lock_path.display()
            ))
        });
    }

    let build_step = ui::Step::start("Running deterministic Docker build");
    let build_status = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/workspace", parent_dir.display()))
        .arg("-w")
        .arg(format!("/workspace/{}", project_name))
        .arg("ellipsislabs/solana:latest")
        .arg("cargo")
        .arg("build-sbf")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    if has_lock {
        fs::rename(&backup_path, &lock_path).unwrap_or_else(|e| {
            ui::error(format!(
                "Cannot restore {} from {}: {e}. Move {} back to {} yourself.",
                lock_path.display(),
                backup_path.display(),
                backup_path.display(),
                lock_path.display()
            ))
        });
    }

    let build_output = match build_status {
        Ok(out) => out,
        Err(e) => build_step.fail(format!("Failed to run Docker: {e}")),
    };
    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr);
        build_step.failed("Dockerized build failed");
        for line in stderr.lines() {
            ui::info(format!("  {line}"));
        }
        std::process::exit(1);
    }
    build_step.done("deterministic build complete");

    let so_path = naclac_client_gen::deploy_dir(&workspace_root).join(format!("{name}.so"));
    let local_bytes = fs::read(&so_path).unwrap_or_else(|e| {
        ui::error(format!(
            "Cannot read the local program {}: {e}. Run `naclac build` first.",
            so_path.display()
        ))
    });
    let local_hash = hex::encode(Sha256::digest(&local_bytes));

    let dump_step = ui::Step::start("Fetching on-chain program bytecode");
    let tmp_onchain_dump = std::env::temp_dir().join(format!("{}_onchain.so", program_id_str));
    let dump_result = Command::new("solana")
        .arg("program")
        .arg("dump")
        .arg(program_id_str)
        .arg(&tmp_onchain_dump)
        .arg("--url")
        .arg(&rpc_url)
        .output();
    let dump = match dump_result {
        Ok(out) => out,
        Err(e) => dump_step.fail(format!("Failed to run `solana program dump`: {e}")),
    };
    if !dump.status.success() {
        dump_step.fail(format!(
            "Failed querying {} — wrong cluster? {}",
            rpc_url,
            String::from_utf8_lossy(&dump.stderr).trim()
        ));
    }

    let onchain_bytes = fs::read(&tmp_onchain_dump).unwrap_or_else(|e| {
        ui::error(format!(
            "Cannot read the fetched on-chain program {}: {e}",
            tmp_onchain_dump.display()
        ))
    });
    if let Err(e) = fs::remove_file(&tmp_onchain_dump) {
        ui::warn(format!(
            "Could not delete {}: {e}",
            tmp_onchain_dump.display()
        ));
    }
    dump_step.done(format!("fetched {} bytes", onchain_bytes.len()));

    // Solana deploys pad `.so` bytecodes to support future reallocs (usually
    // up to 2x the size in zeros), so only the length the local build has is
    // compared.
    let comparable_length = std::cmp::min(onchain_bytes.len(), local_bytes.len());
    let on_chain_hash = hex::encode(Sha256::digest(&onchain_bytes[..comparable_length]));

    ui::kv("local sha256", &local_hash);
    ui::kv("on-chain sha256", &on_chain_hash);

    if local_hash == on_chain_hash {
        ui::milestone(format!(
            "Verification succeeded · on-chain matches local build ({})",
            ui::format_duration(start.elapsed())
        ));
    } else {
        ui::error(format!(
            "Verification failed — on-chain code diverges from local build \
             (local {} vs on-chain {}).",
            local_hash, on_chain_hash
        ));
    }
}
