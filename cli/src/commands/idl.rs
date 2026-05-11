use crate::IdlAction;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use solana_rpc_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_address::Address;

use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_instruction::{Instruction, AccountMeta};
use sha2::{Sha256, Digest};
use std::fs;
use std::io::Write;
use std::str::FromStr;
use toml::Value;

pub fn execute(action: &IdlAction) {
    match action {
        IdlAction::Init { program_id } => init_idl(program_id.as_ref().unwrap()),
    }
}

struct NaclacConfig {
    cluster: String,
    wallet_path: String,
    rpc_url: String,
}

fn load_naclac_config() -> Option<NaclacConfig> {
    let current_dir = std::env::current_dir().unwrap();

    let toml_path = if current_dir.join("Naclac.toml").exists() {
        current_dir.join("Naclac.toml")
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir
            .join("../../Naclac.toml")
            .canonicalize()
            .unwrap()
    } else {
        eprintln!("❌ Not a Naclac workspace. Run `naclac init` to initialize a new one.");
        return None;
    };

    let content = match fs::read_to_string(&toml_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("❌ Found Naclac.toml but could not read it.");
            return None;
        }
    };

    let parsed: Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("❌ Naclac.toml is malformed. Please check its format.");
            return None;
        }
    };

    // Extract [provider] cluster
    let cluster = parsed
        .get("provider")
        .and_then(|p| p.get("cluster"))
        .and_then(|c| c.as_str())
        .unwrap_or("devnet")
        .to_string();

    // Extract [provider] wallet
    let raw_wallet = parsed
        .get("provider")
        .and_then(|p| p.get("wallet"))
        .and_then(|w| w.as_str())
        .unwrap_or("~/.config/solana/id.json")
        .to_string();

    // Expand ~ to actual home directory
    let wallet_path = if raw_wallet.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| {
            // fallback for Windows/WSL
            std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())
        });
        raw_wallet.replacen("~", &home, 1)
    } else {
        raw_wallet
    };

    // Map cluster name to RPC URL
    let rpc_url = match cluster.to_lowercase().as_str() {
        "mainnet" | "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
        "devnet" => "https://api.devnet.solana.com".to_string(),
        "testnet" => "https://api.testnet.solana.com".to_string(),
        "localnet" | "localhost" => "http://127.0.0.1:8899".to_string(),
        // If they put a raw URL directly in the toml, use it as-is
        url if url.starts_with("http") => url.to_string(),
        other => {
            eprintln!(
                "⚠️  Unknown cluster '{}' in Naclac.toml, defaulting to devnet.",
                other
            );
            "https://api.devnet.solana.com".to_string()
        }
    };

    eprintln!(
        "📋 Naclac.toml loaded → cluster: {}, wallet: {}",
        cluster, wallet_path
    );

    Some(NaclacConfig {
        cluster,
        wallet_path,
        rpc_url,
    })
}

fn load_keypair(wallet_path: &str) -> Option<Keypair> {
    let key_bytes = match fs::read_to_string(wallet_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("❌ Could not read wallet keypair at: {}", wallet_path);
            eprintln!("   Make sure the path in Naclac.toml [provider] wallet is correct.");
            return None;
        }
    };

    let bytes: Vec<u8> = match serde_json::from_str(&key_bytes) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("❌ Wallet file is not a valid Solana keypair JSON.");
            return None;
        }
    };

    let secret: [u8; 32] = bytes[..32].try_into().expect("❌ Keypair bytes too short.");
    Some(Keypair::new_from_array(secret))
}

fn init_idl(program_id_str: &String) {
    // 1. Load Naclac.toml config
    let config = match load_naclac_config() {
        Some(c) => c,
        None => return,
    };

    // 2. Parse program ID
    let program_id: Pubkey = match Pubkey::from_str(program_id_str) {
        Ok(pk) => pk,
        Err(_) => {
            eprintln!("❌ Invalid programmatic pubkey string.");
            return;
        }
    };


    eprintln!("🚀 Initialize deterministic IDL context structure for: {}", program_id);

    // 3. Find workspace_root (Needed for Pre-flight Check)
    let current_dir = std::env::current_dir().unwrap();
    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else {
        current_dir.join("../..").canonicalize().unwrap()
    };

    // --- Pre-flight Check: Verify idl-build feature is enabled ---
    if let Some(name) = find_program_name_by_id(&workspace_root, program_id_str, &config.cluster) {
        if !is_idl_feature_enabled(&workspace_root, &name) {
            eprintln!("\n❌ Error: On-chain IDL logic is not enabled for program '{}'.", name);
            eprintln!("   Naclac keeps programs lightweight by default to reduce deployment costs.");
            eprintln!("\nTo enable on-chain IDL storage, add this to your programs/{}/Cargo.toml:", name);
            eprintln!("\n[features]");
            eprintln!("idl-build = [\"naclac-lang/idl-build\"]");
            eprintln!("\nThen rebuild and redeploy your program before running `naclac idl init` again.");
            return;
        }
    }

    // 4. Derive IDL PDA
    let seed = b"anchor:idl";
    let (idl_pda, _bump) = Pubkey::find_program_address(&[seed, program_id.as_ref()], &program_id);
    eprintln!("🔐 Derived Anchor-Compatible IDL PDA: {}", idl_pda);

    // 5. Find and load IDL file

    let idl_dir = workspace_root.join("target/idl");
    let mut idl_content = None;

    if idl_dir.exists() {
        for entry in fs::read_dir(&idl_dir).unwrap() {
            if let Ok(entry) = entry {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains(program_id_str) {
                        idl_content = Some(content);
                        break;
                    }
                }
            }
        }
    }

    let payload = match idl_content {
        Some(c) => c,
        None => {
            eprintln!("❌ Error: Could not find a compiled IDL matched to this program.");
            return;
        }
    };

    // 5. Compress IDL
    eprintln!("🗜  Compressing local target IDL via exact Zlib/deflate standard...");
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload.as_bytes()).unwrap();
    let compressed_bytes = encoder.finish().unwrap();

    let storage_length = 8 + 32 + 4 + compressed_bytes.len();

    eprintln!(
        "✅ Compression Output: ~{} bytes mapped from raw json.",
        compressed_bytes.len()
    );

    eprintln!("💰 IDL Transaction Configured: Rent size evaluation mapped to length: {}.", storage_length);

    // 6. Load keypair from wallet path in Naclac.toml
    let payer = match load_keypair(&config.wallet_path) {
        Some(kp) => kp,
        None => return,
    };


    // 7. Connect to RPC
    eprintln!("🌐 Connecting to cluster: {} ({})", config.cluster, config.rpc_url);
    let client = RpcClient::new(config.rpc_url.clone());

    // 8. Check if IDL account already exists
    let idl_account = client.get_account(&Address::new_from_array(idl_pda.to_bytes())).ok();
    
    if let Some(account) = idl_account {
        let current_size = account.data.len();
        if current_size != storage_length {
            eprintln!("⚠️  IDL account exists but has size {} (required: {}). Resizing...", current_size, storage_length);
            
            let recent_blockhash = match client.get_latest_blockhash() {
                Ok(bh) => bh,
                Err(e) => {
                    eprintln!("❌ Failed to fetch latest blockhash: {}", e);
                    return;
                }
            };

            let idl_resize_disc = {
                let mut hasher = Sha256::new();
                hasher.update(b"naclac:idl_resize");
                hasher.finalize()
            };
            let mut resize_data = idl_resize_disc[0..8].to_vec();
            resize_data.extend_from_slice(&(storage_length as u64).to_le_bytes());

            let resize_ix = Instruction {
                program_id: Address::new_from_array(program_id.to_bytes()),
                accounts: vec![
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new(Address::new_from_array(idl_pda.to_bytes()), false),
                    AccountMeta::new_readonly(Address::new_from_array(solana_system_interface::program::id().to_bytes()), false),
                ],
                data: resize_data,
            };

            let tx = Transaction::new_signed_with_payer(
                &[resize_ix],
                Some(&payer.pubkey()),
                &[&payer],
                recent_blockhash,
            );

            match client.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    eprintln!("✅ IDL resize successful! Signature: {}", sig);
                }
                Err(e) => {
                    eprintln!("❌ Resize transaction failed: {}", e);
                    return;
                }
            }
        } else {
            eprintln!("⚠️  IDL account already exists at {}. Bypassing initialization...", idl_pda);
        }
    } else {
        // 9. Calculate rent
        let rent: u64 = match client.get_minimum_balance_for_rent_exemption(storage_length) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("❌ Failed to fetch rent exemption amount: {}", e);
                return;
            }
        };

        eprintln!("💸 Rent required: {} lamports ({:.6} SOL)", rent, rent as f64 / 1_000_000_000_f64);

        // 10. Check payer balance
        let balance = match client.get_balance(&payer.pubkey()) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("❌ Failed to fetch wallet balance: {}", e);
                return;
            }
        };

        if balance < rent {
            eprintln!(
                "❌ Insufficient balance. You have {} lamports but need {} lamports.",
                balance, rent
            );
            eprintln!("   Run `solana airdrop 1` if you are on devnet.");
            return;
        }

        eprintln!("🔄 Pre-flight checklist completed. Sending initialization transaction...");

        // 11. Build and send create account transaction
        let recent_blockhash = match client.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                eprintln!("❌ Failed to fetch latest blockhash: {}", e);
                return;
            }
        };

        let idl_create_disc = {
            let mut hasher = Sha256::new();
            hasher.update(b"naclac:idl_create");
            hasher.finalize()
        };
        let mut init_data = idl_create_disc[0..8].to_vec();
        init_data.extend_from_slice(&rent.to_le_bytes());
        init_data.extend_from_slice(&(storage_length as u64).to_le_bytes());

        let create_ix = Instruction {
            program_id: Address::new_from_array(program_id.to_bytes()),
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(Address::new_from_array(idl_pda.to_bytes()), false),
                AccountMeta::new_readonly(Address::new_from_array(solana_system_interface::program::id().to_bytes()), false),
            ],
            data: init_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[create_ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        match client.send_and_confirm_transaction(&tx) {
            Ok(sig) => {
                eprintln!("✅ IDL initialize transaction successful!");
                eprintln!("📝 Transaction Signature: {}", sig);
                eprintln!("🔗 View on Explorer: https://explorer.solana.com/tx/{}?cluster={}", sig, config.cluster);
            }
            Err(e) => {
                let err_msg = e.to_string();
                eprintln!("❌ Transaction failed: {}", err_msg);
                if err_msg.contains("invalid instruction data") {
                    eprintln!("\n💡 Tip: This usually means the on-chain program does not include IDL management instructions.");
                    eprintln!("   Ensure you have enabled the 'idl-build' feature in Cargo.toml and REBUILT/REDEPLOYED your program.");
                }
                return; // Exit if init fails
            }
        }
    }

    // 12. Write compressed IDL data in chunks
    // The exact Anchor bytes payload structure: discriminator(8) + authority(32) + length(4) + payload(N)
    let idl_write_disc = {
        let mut hasher = Sha256::new();
        hasher.update(b"naclac:idl_write");
        hasher.finalize()
    };
    let anchor_disc = {
        let mut hasher = Sha256::new();
        hasher.update(b"account:IdlAccount");
        hasher.finalize()
    };

    let mut full_payload = vec![0u8; 8];
    full_payload.copy_from_slice(&anchor_disc[0..8]);
    full_payload.extend_from_slice(payer.pubkey().as_ref());
    full_payload.extend_from_slice(&(compressed_bytes.len() as u32).to_le_bytes());
    full_payload.extend_from_slice(&compressed_bytes);

    let chunk_size = 600;
    let total_len = full_payload.len();
    let mut offset = 0;

    eprintln!("🔄 Starting chunked upload for {} bytes...", total_len);

    while offset < total_len {
        let end = std::cmp::min(offset + chunk_size, total_len);
        let chunk = &full_payload[offset..end];

        let mut write_data = idl_write_disc[0..8].to_vec();
        write_data.extend_from_slice(&(offset as u32).to_le_bytes());
        write_data.extend_from_slice(chunk);

        let write_ix = Instruction {
            program_id: Address::new_from_array(program_id.to_bytes()),
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(Address::new_from_array(idl_pda.to_bytes()), false),
            ],
            data: write_data,
        };

        let current_blockhash = match client.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                eprintln!("❌ Failed to fetch latest blockhash for chunk: {}", e);
                return;
            }
        };

        let chunk_tx = Transaction::new_signed_with_payer(
            &[write_ix],
            Some(&payer.pubkey()),
            &[&payer],
            current_blockhash,
        );

        match client.send_and_confirm_transaction(&chunk_tx) {
            Ok(_) => {
                eprintln!("✅ Programmed confirmed onchain: {}/{} bytes", end, total_len);
            }
            Err(e) => {
                eprintln!("❌ Write chunk failed at offset {}: {}", offset, e);
                return;
            }
        }
        offset += chunk_size;
    }

    eprintln!("🎉 IDL successfully deployed and is 100% Anchor-Compatible!");
}
fn find_program_name_by_id(workspace_root: &std::path::Path, id: &str, cluster: &str) -> Option<String> {
    let toml_path = workspace_root.join("Naclac.toml");
    let content = fs::read_to_string(toml_path).ok()?;
    let parsed: Value = toml::from_str(&content).ok()?;

    let programs = parsed.get("programs")?.get(cluster)?.as_table()?;
    for (name, val) in programs {
        if val.as_str()? == id {
            return Some(name.clone());
        }
    }
    None
}

fn is_idl_feature_enabled(workspace_root: &std::path::Path, program_name: &str) -> bool {
    let toml_path = workspace_root.join("programs").join(program_name).join("Cargo.toml");
    let content = match fs::read_to_string(toml_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let parsed: Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if let Some(features) = parsed.get("features").and_then(|f| f.as_table()) {
        return features.contains_key("idl-build");
    }
    false
}
