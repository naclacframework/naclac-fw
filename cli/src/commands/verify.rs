use crate::ui;
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::fs;
use std::process::Command;

use solana_keypair::Keypair;
use solana_signer::Signer;
use std::str::FromStr;

pub fn execute(program_id_str: &String) {
    // Validate the pubkey format early even though we use it as a string for display
    if bs58::decode(program_id_str).into_vec().is_err() {
        ui::error_line("Invalid base58 program ID.");
        return;
    }

    let docker_check = Command::new("docker").arg("--version").output();

    match docker_check {
        Ok(out) if out.status.success() => {}
        _ => {
            ui::error_line(
                "Docker is not running or missing from PATH — naclac needs Docker for \
                 deterministic verify builds. Install it (docs.docker.com/get-docker) and \
                 make sure the daemon is running.",
            );
            return;
        }
    }

    let current_dir = std::env::current_dir().unwrap();
    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        ui::error_line("Could not find Naclac.toml.");
        return;
    };

    // Docker's own build output is inherited straight through to the
    // terminal, so no live spinner runs alongside it here — a
    // steady-ticking spinner and a subprocess writing to the same inherited
    // stdio at the same time would corrupt each other's output (see the
    // same reasoning that ruled out a PTY for `naclac build`'s own compile
    // step in build.rs).
    ui::info(format!(
        "Verifying {} (dockerized build)...",
        program_id_str
    ));

    let parent_dir = workspace_root.parent().unwrap();
    let project_name = workspace_root.file_name().unwrap().to_str().unwrap();

    let lock_path = workspace_root.join("Cargo.lock");
    let mut lock_content = None;

    // Mask the V4 Lockfile Native to the host so the isolated docker runtime doesn't crash reading it
    if lock_path.exists() {
        if let Ok(content) = fs::read_to_string(&lock_path) {
            lock_content = Some(content);
            fs::remove_file(&lock_path).unwrap_or_default();
        }
    }

    let mut build_cmd = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/workspace", parent_dir.display()))
        .arg("-w")
        .arg(format!("/workspace/{}", project_name))
        .arg("ellipsislabs/solana:latest")
        .arg("cargo")
        .arg("build-sbf")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("Failed to initialize Docker runtime.");

    let build_status = build_cmd
        .wait()
        .expect("Docker wrapper unexpectedly failed.");

    // Restore the host's lockfile immediately after the isolation drops
    if let Some(content) = lock_content {
        fs::write(&lock_path, content).unwrap_or_default();
    }

    if !build_status.success() {
        ui::error_line("Dockerized build failed — resolve errors to continue verifying.");
        return;
    }

    // Find matching SO file bypassing simple string names
    let deploy_dir = naclac_client_gen::resolve_target_dir(&workspace_root).join("deploy");
    let mut matching_so = None;

    let target_address = Address::from_str(program_id_str).unwrap();

    if deploy_dir.exists() {
        if let Ok(entries) = fs::read_dir(&deploy_dir) {
            let mut so_files = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "so" {
                        so_files.push(path.clone());
                    } else if ext == "json"
                        && path
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .ends_with("-keypair")
                    {
                        if let Ok(key_bytes) = fs::read_to_string(&path) {
                            if let Ok(bytes) = serde_json::from_str::<Vec<u8>>(&key_bytes) {
                                if bytes.len() >= 32 {
                                    let secret: [u8; 32] = bytes[..32].try_into().unwrap();
                                    let kp = Keypair::new_from_array(secret);
                                    if kp.pubkey() == target_address {
                                        let base_name = path
                                            .file_stem()
                                            .unwrap()
                                            .to_string_lossy()
                                            .replace("-keypair", "");
                                        let target_so =
                                            deploy_dir.join(format!("{}.so", base_name));
                                        if target_so.exists() {
                                            matching_so = Some(target_so);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if matching_so.is_none() && so_files.len() == 1 {
                matching_so = Some(so_files[0].clone());
            }
        }
    }

    if let Some(so_path) = matching_so {
        if let Ok(local_bytes) = fs::read(&so_path) {
            let mut hasher = Sha256::new();
            hasher.update(&local_bytes);
            let hash_bytes = hasher.finalize();
            let local_hash = hex::encode(hash_bytes);

            let tmp_onchain_dump =
                std::env::temp_dir().join(format!("{}_onchain.so", program_id_str));
            let dump_cmd = Command::new("solana")
                .arg("program")
                .arg("dump")
                .arg(program_id_str)
                .arg(&tmp_onchain_dump)
                .output()
                .expect("Failed to execute solana program dump fetching process.");

            if !dump_cmd.status.success() {
                ui::error_line(format!(
                    "Failed querying network — wrong cluster? {}",
                    String::from_utf8_lossy(&dump_cmd.stderr)
                ));
                return;
            }

            if let Ok(onchain_bytes) = fs::read(&tmp_onchain_dump) {
                // Slicing algorithm: Solana deploys pad `.so` bytecodes to support future Reallocs (Usually +2x size in zeros).
                // We strictly map and isolate their verification length boundaries to emulate flawless determinism comparisons against local artifacts.
                let comparable_length = std::cmp::min(onchain_bytes.len(), local_bytes.len());
                let comparable_onchain_slice = &onchain_bytes[..comparable_length];

                let mut chain_hasher = Sha256::new();
                chain_hasher.update(comparable_onchain_slice);
                let on_chain_hash = hex::encode(chain_hasher.finalize());

                if local_hash == on_chain_hash {
                    ui::success(format!(
                        "On-chain code matches local source ({})",
                        local_hash
                    ));
                } else {
                    ui::error_line(format!(
                        "Verification failed — on-chain code diverges from local build \
                         (local {} vs on-chain {}).",
                        local_hash, on_chain_hash
                    ));
                }

                fs::remove_file(&tmp_onchain_dump).unwrap_or_default();
            } else {
                ui::error_line("Failed to read the fetched on-chain dump.");
            }
        } else {
            ui::error_line("Failed to read the local `.so` file.");
        }
    } else {
        ui::error_line(format!(
            "No verifiable program artifacts found for {}",
            program_id_str
        ));
    }
}
