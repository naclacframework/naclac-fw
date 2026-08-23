//! `naclac idl <action>` — manages a program's on-chain IDL metadata via
//! Solana's Program Metadata Program, through `naclac_idl::program_metadata`:
//! `upload` (create/update), `set-immutable`, `close`, `trim`, and
//! `set-authority`.

use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression as ZlibCompression;
use naclac_idl::program_metadata::{
    self, compression, data_source, encoding, format, header, MetadataAccounts, Seed,
    SetDataSource, HEADER_LEN,
};
use solana_address::Address;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_rpc_client::rpc_client::RpcClient;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::fs;
use std::io::Read as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml::Value;

use crate::ui;

/// Conservative per-transaction payload budget for `write`/inline calls —
/// Solana caps total transaction size at 1232 bytes; this leaves headroom
/// for signatures, account keys, and the fixed instruction header.
const CHUNK_SIZE: usize = 800;

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
        ui::error_line("Not a Naclac workspace. Run `naclac init` to initialize a new one.");
        return None;
    };

    let content = match fs::read_to_string(&toml_path) {
        Ok(c) => c,
        Err(_) => {
            ui::error_line("Found Naclac.toml but could not read it.");
            return None;
        }
    };

    let parsed: Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            ui::error_line("Naclac.toml is malformed — check its format.");
            return None;
        }
    };

    let cluster = parsed
        .get("provider")
        .and_then(|p| p.get("cluster"))
        .and_then(|c| c.as_str())
        .unwrap_or("devnet")
        .to_string();

    let raw_wallet = parsed
        .get("provider")
        .and_then(|p| p.get("wallet"))
        .and_then(|w| w.as_str())
        .unwrap_or("~/.config/solana/id.json")
        .to_string();

    let wallet_path = if raw_wallet.starts_with("~/") {
        let home = std::env::var("HOME")
            .unwrap_or_else(|_| std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()));
        raw_wallet.replacen("~", &home, 1)
    } else {
        raw_wallet
    };

    let rpc_url = match cluster.to_lowercase().as_str() {
        "mainnet" | "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
        "devnet" => "https://api.devnet.solana.com".to_string(),
        "testnet" => "https://api.testnet.solana.com".to_string(),
        "localnet" | "localhost" => "http://127.0.0.1:8899".to_string(),
        url if url.starts_with("http") => url.to_string(),
        other => {
            ui::warn(format!(
                "Unknown cluster '{}' in Naclac.toml — defaulting to devnet.",
                other
            ));
            "https://api.devnet.solana.com".to_string()
        }
    };

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
            ui::error_line(format!(
                "Could not read wallet keypair at {} — check [provider] wallet in Naclac.toml.",
                wallet_path
            ));
            return None;
        }
    };

    let bytes: Vec<u8> = match serde_json::from_str(&key_bytes) {
        Ok(b) => b,
        Err(_) => {
            ui::error_line("Wallet file is not a valid Solana keypair JSON.");
            return None;
        }
    };

    let secret: [u8; 32] = bytes[..32].try_into().expect("❌ Keypair bytes too short.");
    Some(Keypair::new_from_array(secret))
}

fn workspace_root() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap();
    if current_dir.join("Naclac.toml").exists() {
        current_dir
    } else {
        current_dir.join("../..").canonicalize().unwrap()
    }
}

fn find_program_name(workspace_root: &Path, id: &str, cluster: &str) -> Option<String> {
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

/// True if it's safe to proceed — `naclac idl` commands only support
/// devnet/testnet/mainnet, since the Program Metadata Program isn't
/// reliably usable on a local validator (it isn't in the default genesis at
/// all, and even cloned in, its System Program CPIs can mismatch an older
/// local validator build).
fn require_non_local_cluster(config: &NaclacConfig) -> bool {
    if matches!(
        config.cluster.to_lowercase().as_str(),
        "localnet" | "localhost"
    ) {
        ui::error_line(
            "`naclac idl` commands only support devnet/testnet/mainnet — set [provider] \
             cluster in Naclac.toml to \"devnet\" or higher.",
        );
        return false;
    }
    true
}

pub fn execute_upload(program_id_str: &str) {
    let config = match load_naclac_config() {
        Some(c) => c,
        None => return,
    };

    if !require_non_local_cluster(&config) {
        return;
    }

    let program_id = match Address::from_str(program_id_str) {
        Ok(pk) => pk,
        Err(_) => {
            ui::error_line("Invalid program address string.");
            return;
        }
    };

    let root = workspace_root();

    let program_name = match find_program_name(&root, program_id_str, &config.cluster) {
        Some(name) => name,
        None => {
            ui::error_line(format!(
                "Could not find program '{}' in Naclac.toml under [programs.{}].",
                program_id_str, config.cluster
            ));
            return;
        }
    };

    let idl_path = naclac_client_gen::resolve_target_dir(&root)
        .join("idl")
        .join(format!("{}.json", program_name));
    let idl_content = match fs::read_to_string(&idl_path) {
        Ok(c) => c,
        Err(_) => {
            ui::error_line(format!(
                "No compiled IDL found at {} — run `naclac build` first.",
                idl_path.display()
            ));
            return;
        }
    };

    let mut encoder = ZlibEncoder::new(Vec::new(), ZlibCompression::default());
    encoder.write_all(idl_content.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();

    let seed = program_metadata::seed_from_str("idl");
    let (metadata_pda, _bump) = program_metadata::derive_canonical_metadata_pda(&program_id, &seed);
    let (program_data, _bump) = program_metadata::derive_program_data_address(&program_id);

    let payer = match load_keypair(&config.wallet_path) {
        Some(kp) => kp,
        None => return,
    };
    let authority = payer.pubkey();

    let client = RpcClient::new(config.rpc_url.clone());

    let already_exists = client.get_account(&metadata_pda).is_ok();

    if already_exists {
        update_existing(
            &client,
            &payer,
            &program_id,
            &program_data,
            &metadata_pda,
            &authority,
            &compressed,
        );
    } else {
        let accounts = MetadataAccounts {
            metadata: &metadata_pda,
            authority: &authority,
            program: &program_id,
            program_data: &program_data,
        };
        create_new(&client, &payer, accounts, &seed, &compressed);
    }
}

/// Tops up `target`'s lamport balance to at least the rent-exempt minimum
/// for `size` bytes, if it isn't there already. Required before
/// `allocate`/`initialize`/`set_data`: none of them transfer lamports
/// themselves — the on-chain program's own account-validation comments are
/// explicit that the caller must pre-fund the account first, and the
/// runtime enforces rent-exemption on any resize regardless.
fn ensure_rent_exempt(client: &RpcClient, payer: &Keypair, target: &Address, size: usize) -> bool {
    let required = match client.get_minimum_balance_for_rent_exemption(size) {
        Ok(r) => r,
        Err(e) => {
            ui::error_line(format!("Failed to fetch rent-exemption amount: {}", e));
            return false;
        }
    };
    let current = client.get_balance(target).unwrap_or(0);
    if current >= required {
        return true;
    }
    let ix =
        solana_system_interface::instruction::transfer(&payer.pubkey(), target, required - current);
    send(client, payer, ix, "Fund account for rent-exemption")
}

fn send(client: &RpcClient, payer: &Keypair, ix: Instruction, label: &str) -> bool {
    let recent_blockhash = match client.get_latest_blockhash() {
        Ok(bh) => bh,
        Err(e) => {
            ui::error_line(format!("Failed to fetch latest blockhash: {}", e));
            return false;
        }
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    match client.send_and_confirm_transaction(&tx) {
        Ok(sig) => {
            ui::success(format!("{} — {}", label, sig));
            true
        }
        Err(e) => {
            ui::error_line(format!("{} failed: {}", label, e));
            false
        }
    }
}

fn write_chunks(
    client: &RpcClient,
    payer: &Keypair,
    buffer: &Address,
    authority: &Address,
    data: &[u8],
) -> bool {
    let mut offset = 0u32;
    let total = data.len();
    while (offset as usize) < total {
        let end = std::cmp::min(offset as usize + CHUNK_SIZE, total);
        let chunk = &data[offset as usize..end];
        let ix = program_metadata::write(buffer, authority, offset, chunk);
        if !send(
            client,
            payer,
            ix,
            &format!("Write ({}/{} bytes)", end, total),
        ) {
            return false;
        }
        offset = end as u32;
    }
    true
}

fn create_new(
    client: &RpcClient,
    payer: &Keypair,
    accounts: MetadataAccounts<'_>,
    seed: &Seed,
    compressed: &[u8],
) {
    let metadata_pda = *accounts.metadata;
    let final_size = HEADER_LEN + compressed.len();

    if compressed.len() <= CHUNK_SIZE {
        if !ensure_rent_exempt(client, payer, accounts.metadata, final_size) {
            return;
        }
        let ix = program_metadata::initialize(
            accounts,
            seed,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
            Some(compressed),
        );
        if send(client, payer, ix, "Initialize (inline)") {
            ui::success(format!("IDL uploaded ({})", metadata_pda));
        }
        return;
    }

    // Large payload: fund the metadata PDA upfront for its *final* size
    // (avoids needing per-chunk top-ups as `write` grows it), allocate it
    // as a buffer, write it in chunks, then finalize with an empty-data
    // initialize.
    if !ensure_rent_exempt(client, payer, accounts.metadata, final_size) {
        return;
    }
    let allocate_ix = program_metadata::allocate(
        accounts.metadata,
        accounts.authority,
        accounts.program,
        accounts.program_data,
        seed,
    );
    if !send(client, payer, allocate_ix, "Allocate") {
        return;
    }

    if !write_chunks(
        client,
        payer,
        accounts.metadata,
        accounts.authority,
        compressed,
    ) {
        return;
    }

    let init_ix = program_metadata::initialize(
        accounts,
        seed,
        encoding::UTF8,
        compression::ZLIB,
        format::JSON,
        data_source::DIRECT,
        None,
    );
    if send(client, payer, init_ix, "Initialize (finalize buffer)") {
        ui::success(format!("IDL uploaded ({})", metadata_pda));
    }
}

fn update_existing(
    client: &RpcClient,
    payer: &Keypair,
    program_id: &Address,
    program_data: &Address,
    metadata_pda: &Address,
    authority: &Address,
    compressed: &[u8],
) {
    let accounts = MetadataAccounts {
        metadata: metadata_pda,
        authority,
        program: program_id,
        program_data,
    };

    let final_size = HEADER_LEN + compressed.len();

    if compressed.len() <= CHUNK_SIZE {
        // Only actually transfers anything if the new (possibly larger)
        // payload needs more than the account currently holds.
        if !ensure_rent_exempt(client, payer, metadata_pda, final_size) {
            return;
        }
        let ix = program_metadata::set_data(
            accounts,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            SetDataSource::Inline {
                data_source: data_source::DIRECT,
                data: compressed,
            },
        );
        if send(client, payer, ix, "SetData (inline)") {
            ui::success(format!("IDL updated ({})", metadata_pda));
        }
        return;
    }

    // Large update: the real metadata PDA is already occupied, so write
    // into a separate temporary buffer PDA instead, point set_data at it,
    // then close the buffer to reclaim its rent.
    let buffer_seed = program_metadata::seed_from_str("idl-buffer");
    let (buffer_pda, _bump) =
        program_metadata::derive_canonical_metadata_pda(program_id, &buffer_seed);

    if !ensure_rent_exempt(client, payer, &buffer_pda, final_size) {
        return;
    }
    let allocate_ix = program_metadata::allocate(
        &buffer_pda,
        authority,
        program_id,
        program_data,
        &buffer_seed,
    );
    if !send(client, payer, allocate_ix, "Allocate (temp buffer)") {
        return;
    }

    if !write_chunks(client, payer, &buffer_pda, authority, compressed) {
        return;
    }

    let set_data_ix = program_metadata::set_data(
        accounts,
        encoding::UTF8,
        compression::ZLIB,
        format::JSON,
        SetDataSource::FromBuffer {
            data_source: data_source::DIRECT,
            buffer: buffer_pda,
        },
    );
    if !send(client, payer, set_data_ix, "SetData (from buffer)") {
        return;
    }

    let close_ix =
        program_metadata::close(&buffer_pda, authority, program_id, program_data, authority);
    if send(client, payer, close_ix, "Close (temp buffer cleanup)") {
        ui::success(format!("IDL updated ({})", metadata_pda));
    }
}

/// Resolves the shared config/keypair/PDA/RPC-client setup every `naclac
/// idl <action>` command (besides `upload`, which has its own extra IDL-
/// compression steps) needs. `None` on any failure — the callee has already
/// printed the reason.
fn common_setup(
    program_id_str: &str,
) -> Option<(NaclacConfig, Address, Address, Address, Keypair, RpcClient)> {
    let config = load_naclac_config()?;
    if !require_non_local_cluster(&config) {
        return None;
    }

    let program_id = match Address::from_str(program_id_str) {
        Ok(pk) => pk,
        Err(_) => {
            ui::error_line("Invalid program address string.");
            return None;
        }
    };

    let seed = program_metadata::seed_from_str("idl");
    let (metadata_pda, _bump) = program_metadata::derive_canonical_metadata_pda(&program_id, &seed);
    let (program_data, _bump) = program_metadata::derive_program_data_address(&program_id);

    let payer = load_keypair(&config.wallet_path)?;

    let client = RpcClient::new(config.rpc_url.clone());

    Some((
        config,
        program_id,
        metadata_pda,
        program_data,
        payer,
        client,
    ))
}

pub fn execute_set_immutable(program_id_str: &str) {
    let Some((_config, program_id, metadata_pda, program_data, payer, client)) =
        common_setup(program_id_str)
    else {
        return;
    };
    let authority = payer.pubkey();

    let ix = program_metadata::set_immutable(&metadata_pda, &authority, &program_id, &program_data);
    if send(&client, &payer, ix, "SetImmutable") {
        ui::success(format!(
            "IDL metadata is now permanently immutable ({})",
            metadata_pda
        ));
    }
}

pub fn execute_close(program_id_str: &str, destination: Option<&str>) {
    let Some((_config, program_id, metadata_pda, program_data, payer, client)) =
        common_setup(program_id_str)
    else {
        return;
    };
    let authority = payer.pubkey();

    let destination_addr = match destination {
        Some(d) => match Address::from_str(d) {
            Ok(addr) => addr,
            Err(_) => {
                ui::error_line("Invalid destination address string.");
                return;
            }
        },
        None => authority,
    };

    let ix = program_metadata::close(
        &metadata_pda,
        &authority,
        &program_id,
        &program_data,
        &destination_addr,
    );
    if send(&client, &payer, ix, "Close") {
        ui::success(format!("IDL metadata account closed ({})", metadata_pda));
    }
}

pub fn execute_trim(program_id_str: &str, destination: Option<&str>) {
    let Some((_config, program_id, metadata_pda, program_data, payer, client)) =
        common_setup(program_id_str)
    else {
        return;
    };
    let authority = payer.pubkey();

    let destination_addr = match destination {
        Some(d) => match Address::from_str(d) {
            Ok(addr) => addr,
            Err(_) => {
                ui::error_line("Invalid destination address string.");
                return;
            }
        },
        None => authority,
    };

    let ix = program_metadata::trim(
        &metadata_pda,
        &authority,
        &program_id,
        &program_data,
        &destination_addr,
    );
    if send(&client, &payer, ix, "Trim") {
        ui::success(format!("IDL metadata account trimmed ({})", metadata_pda));
    }
}

pub fn execute_set_authority(program_id_str: &str, new_authority: Option<&str>, remove: bool) {
    if remove && new_authority.is_some() {
        ui::error_line("Pass either --new-authority or --remove, not both.");
        return;
    }
    if !remove && new_authority.is_none() {
        ui::error_line(
            "Pass --new-authority <PUBKEY> to set a new authority, or --remove to remove it.",
        );
        return;
    }

    let new_authority_addr = match new_authority {
        Some(a) => match Address::from_str(a) {
            Ok(addr) => Some(addr),
            Err(_) => {
                ui::error_line("Invalid new-authority address string.");
                return;
            }
        },
        None => None,
    };

    let Some((_config, program_id, metadata_pda, program_data, payer, client)) =
        common_setup(program_id_str)
    else {
        return;
    };
    let authority = payer.pubkey();

    let label = match &new_authority_addr {
        Some(a) => format!("SetAuthority (-> {})", a),
        None => "SetAuthority (remove)".to_string(),
    };
    let ix = program_metadata::set_authority(
        &metadata_pda,
        &authority,
        &program_id,
        &program_data,
        new_authority_addr.as_ref(),
    );
    if send(&client, &payer, ix, &label) {
        ui::success("Authority updated.");
    }
}

pub fn execute_fetch(program_id_str: &str, output: Option<&str>) {
    let config = match load_naclac_config() {
        Some(c) => c,
        None => return,
    };

    if !require_non_local_cluster(&config) {
        return;
    }

    let program_id = match Address::from_str(program_id_str) {
        Ok(pk) => pk,
        Err(_) => {
            ui::error_line("Invalid program address string.");
            return;
        }
    };

    let seed = program_metadata::seed_from_str("idl");
    let (metadata_pda, _bump) = program_metadata::derive_canonical_metadata_pda(&program_id, &seed);

    let client = RpcClient::new(config.rpc_url.clone());

    let account_data = match client.get_account_data(&metadata_pda) {
        Ok(data) => data,
        Err(_) => {
            ui::error_line(format!(
                "No metadata account found at {} — has this program's IDL been uploaded?",
                metadata_pda
            ));
            return;
        }
    };

    let parsed = match header::parse_header(&account_data) {
        Some(h) => h,
        None => {
            ui::error_line(format!(
                "Metadata account {} has an unrecognized layout.",
                metadata_pda
            ));
            return;
        }
    };

    let raw_payload = match header::payload(&account_data, &parsed) {
        Some(p) => p,
        None => {
            ui::error_line(format!(
                "Metadata account {} is truncated or corrupt.",
                metadata_pda
            ));
            return;
        }
    };

    let mut decompressed = Vec::new();
    let decompress_result = match parsed.compression {
        compression::NONE => {
            decompressed.extend_from_slice(raw_payload);
            Ok(())
        }
        compression::GZIP => GzDecoder::new(raw_payload)
            .read_to_end(&mut decompressed)
            .map(|_| ()),
        compression::ZLIB => ZlibDecoder::new(raw_payload)
            .read_to_end(&mut decompressed)
            .map(|_| ()),
        other => {
            ui::error_line(format!(
                "Unrecognized compression byte {} on metadata account.",
                other
            ));
            return;
        }
    };
    if let Err(e) = decompress_result {
        ui::error_line(format!("Failed to decompress IDL payload: {}", e));
        return;
    }

    let default_path = || {
        let root = workspace_root();
        let name = find_program_name(&root, program_id_str, &config.cluster)?;
        Some(
            naclac_client_gen::resolve_target_dir(&root)
                .join("idl")
                .join(format!("{}.onchain.json", name)),
        )
    };

    match output.map(PathBuf::from).or_else(default_path) {
        Some(path) => {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::write(&path, &decompressed) {
                Ok(_) => ui::success(format!("IDL written to {}", path.display())),
                Err(e) => ui::error_line(format!("Failed to write to {}: {}", path.display(), e)),
            }
        }
        None => {
            print!("{}", String::from_utf8_lossy(&decompressed));
        }
    }
}
