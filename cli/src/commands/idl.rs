//! `naclac idl <action>` — manages a program's on-chain IDL metadata via
//! Solana's Program Metadata Program, through `naclac_idl::program_metadata`:
//! `upload` (create/update), `fetch`, `set-immutable`, `close`, `trim`, and
//! `set-authority`. Every action returns its failure as an `Err` so the
//! caller can exit non-zero.

use {
    crate::{
        commands::{keypair, workspace},
        ui,
    },
    dialoguer::{theme::ColorfulTheme, Confirm},
    flate2::{
        read::{GzDecoder, ZlibDecoder},
        write::ZlibEncoder,
        Compression as ZlibCompression,
    },
    naclac_idl::program_metadata::{
        self, compression, data_source, encoding, format, header, MetadataAccounts, Seed,
        SetDataSource, HEADER_LEN,
    },
    solana_address::Address,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_rpc_client::rpc_client::RpcClient,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{
        fs,
        io::{Read as _, Write as _},
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

type Outcome = Result<(), String>;

/// Conservative per-transaction payload budget for `write`/inline calls —
/// Solana caps total transaction size at 1232 bytes; this leaves headroom
/// for signatures, account keys, and the fixed instruction header.
const CHUNK_SIZE: usize = 800;

/// Folder under `artifacts/` that `naclac idl fetch` writes to unless `--dir`
/// is given. Kept apart from `artifacts/idl`, which holds the IDLs generated
/// from the local source under the same file names.
const DEFAULT_FETCH_DIR: &str = "onchain-idl";

/// Largest decompressed IDL `naclac idl fetch` accepts — the maximum size of
/// a Solana account.
const MAX_IDL_BYTES: u64 = 10 * 1024 * 1024;

/// `UpgradeableLoaderState::ProgramData`'s variant tag, and the byte length
/// of its serialized fields (4-byte tag, 8-byte slot, 1-byte option tag,
/// 32-byte upgrade authority).
const PROGRAM_DATA_TAG: u32 = 3;
const PROGRAM_DATA_METADATA_LEN: usize = 45;

struct NaclacConfig {
    cluster: String,
    wallet_path: PathBuf,
    rpc_url: String,
}

fn load_naclac_config() -> NaclacConfig {
    let root = workspace::root();
    let config = workspace::config(&root);
    let cluster = workspace::cluster(&config);
    NaclacConfig {
        rpc_url: workspace::rpc_url(&cluster),
        wallet_path: workspace::wallet(&config),
        cluster,
    }
}

/// The owner, data and executable flag of an account that exists on chain.
struct OnChainAccount {
    owner: Address,
    data: Vec<u8>,
    executable: bool,
}

/// `Ok(None)` only when the cluster reports the account does not exist; any
/// RPC failure is an `Err`.
fn fetch_account(client: &RpcClient, address: &Address) -> Result<Option<OnChainAccount>, String> {
    let response = retry_read(&format!("read account {address}"), || {
        client
            .get_multiple_accounts_with_commitment(&[*address], client.commitment())
            .map_err(|e| format!("Failed to read account {address}: {e}"))
    })?;
    let [account] = <[_; 1]>::try_from(response.value).map_err(|accounts| {
        format!(
            "Asked the cluster for one account ({address}) and got {}.",
            accounts.len()
        )
    })?;
    Ok(account.map(|account| OnChainAccount {
        owner: account.owner,
        data: account.data,
        executable: account.executable,
    }))
}

/// Total tries, first attempt included, for any RPC read or transaction send.
const MAX_ATTEMPTS: u32 = 5;

/// 1s, 2s, 4s, 8s after the first, second, third and fourth failed attempt.
fn backoff(base: Duration, failed_attempts: u32) -> Duration {
    base * (1u32 << (failed_attempts - 1))
}

/// Runs `read`, repeating it after a failure up to [`MAX_ATTEMPTS`] times in
/// total with exponential backoff. Only for operations that change nothing on
/// chain, since a failure may mean the request was received.
fn retry_read<T>(what: &str, read: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    retry_read_with(Duration::from_secs(1), what, read)
}

fn retry_read_with<T>(
    base: Duration,
    what: &str,
    mut read: impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut attempt = 1;
    loop {
        match read() {
            Ok(value) => return Ok(value),
            Err(e) if attempt < MAX_ATTEMPTS => {
                ui::warn(format!(
                    "{what}: {e} — retrying ({attempt}/{})",
                    MAX_ATTEMPTS - 1
                ));
                std::thread::sleep(backoff(base, attempt));
                attempt += 1;
            }
            Err(e) => return Err(format!("{e} (gave up after {MAX_ATTEMPTS} attempts)")),
        }
    }
}

fn parse_address(what: &str, text: &str) -> Result<Address, String> {
    Address::from_str(text).map_err(|_| format!("`{text}` is not a valid {what} address."))
}

/// The name of the program listed at `id` in `[programs.<cluster>]`, if any.
fn find_program_name(workspace_root: &Path, id: &str, cluster: &str) -> Option<String> {
    let config = workspace::config(workspace_root);
    let programs = config.get("programs")?.get(cluster)?.as_table()?;
    programs
        .iter()
        .find(|(_, address)| address.as_str() == Some(id))
        .map(|(name, _)| name.clone())
}

/// Errors unless both the Program Metadata Program and `program_id` exist on
/// the cluster as executable programs. A local validator doesn't have the
/// Program Metadata Program unless it was started with it.
fn require_programs_loaded(client: &RpcClient, cluster: &str, program_id: &Address) -> Outcome {
    let metadata_program = program_metadata::PROGRAM_METADATA_ID;
    match fetch_account(client, &metadata_program)? {
        Some(account) if account.executable => {}
        _ => {
            return Err(format!(
                "The Program Metadata Program ({metadata_program}) is not loaded on {cluster}. \
                 On a local validator, start it with `--clone-upgradeable-program \
                 {metadata_program} --url devnet --reset`."
            ))
        }
    }
    match fetch_account(client, program_id)? {
        Some(account) if account.executable => Ok(()),
        Some(_) => Err(format!(
            "{program_id} exists on {cluster} but is not an executable program."
        )),
        None => Err(format!(
            "Program {program_id} is not deployed on {cluster} — run `naclac deploy` first."
        )),
    }
}

/// Everything a state-changing `naclac idl` action needs for one program: the
/// wallet that signs, the RPC connection, and the program's metadata and
/// program-data addresses.
struct Session {
    cluster: String,
    program_id: Address,
    metadata_pda: Address,
    program_data: Address,
    payer: Keypair,
    client: RpcClient,
}

impl Session {
    fn open(program_id_str: &str) -> Result<Self, String> {
        let config = load_naclac_config();
        let program_id = parse_address("program", program_id_str)?;
        let seed = program_metadata::seed_from_str("idl");
        let (metadata_pda, _bump) =
            program_metadata::derive_canonical_metadata_pda(&program_id, &seed);
        let (program_data, _bump) = program_metadata::derive_program_data_address(&program_id);
        let payer = keypair::read(&config.wallet_path)?;
        let client = RpcClient::new(config.rpc_url);
        require_programs_loaded(&client, &config.cluster, &program_id)?;
        Ok(Self {
            cluster: config.cluster,
            program_id,
            metadata_pda,
            program_data,
            payer,
            client,
        })
    }

    fn authority(&self) -> Address {
        self.payer.pubkey()
    }
}

/// The program's upgrade authority recorded in its program-data account, or
/// `None` when the program is immutable.
fn parse_upgrade_authority(
    account: &OnChainAccount,
    program_data: &Address,
) -> Result<Option<Address>, String> {
    let malformed =
        || format!("{program_data} is not a valid upgradeable-loader program-data account.");
    if account.owner != program_metadata::BPF_LOADER_UPGRADEABLE_ID
        || account.data.len() < PROGRAM_DATA_METADATA_LEN
    {
        return Err(malformed());
    }
    let tag = u32::from_le_bytes(account.data[0..4].try_into().map_err(|_| malformed())?);
    if tag != PROGRAM_DATA_TAG {
        return Err(malformed());
    }
    match account.data[12] {
        0 => Ok(None),
        1 => {
            let key: [u8; 32] = account.data[13..45].try_into().map_err(|_| malformed())?;
            Ok(Some(Address::new_from_array(key)))
        }
        _ => Err(malformed()),
    }
}

/// Errors unless the wallet is allowed to manage the program's canonical
/// metadata: the Program Metadata Program accepts the program's upgrade
/// authority, or the authority the metadata account itself names.
fn require_authorized(session: &Session) -> Outcome {
    let wallet = session.authority();
    let program_data = fetch_account(&session.client, &session.program_data)?.ok_or_else(|| {
        format!(
            "Program {} has no program-data account on {}: it is not deployed there, or it is \
             not an upgradeable program.",
            session.program_id, session.cluster
        )
    })?;
    let upgrade_authority = parse_upgrade_authority(&program_data, &session.program_data)?;
    if upgrade_authority == Some(wallet) {
        return Ok(());
    }
    if let Some(account) = fetch_account(&session.client, &session.metadata_pda)? {
        let named_authority = header::parse_header(&account.data).and_then(|h| h.authority);
        if account.owner == program_metadata::PROGRAM_METADATA_ID && named_authority == Some(wallet)
        {
            return Ok(());
        }
    }
    let upgrade_authority = match upgrade_authority {
        Some(authority) => authority.to_string(),
        None => "none (the program is immutable)".to_string(),
    };
    Err(format!(
        "Wallet {wallet} may not manage the IDL of {}: the program's upgrade authority is \
         {upgrade_authority}, and the metadata account names no authority for this wallet.",
        session.program_id
    ))
}

/// Asks the user to confirm an irreversible action. Without `yes` and
/// without a terminal to ask on, this is an error, never an implicit yes.
fn confirm_irreversible(question: &str, yes: bool) -> Outcome {
    if yes {
        return Ok(());
    }
    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(question)
        .default(false)
        .interact()
        .map_err(|e| format!("Cannot ask for confirmation ({e}); re-run with --yes to confirm."))?;
    if confirmed {
        Ok(())
    } else {
        Err("Cancelled; nothing was changed.".to_string())
    }
}

/// True only if `address` already holds a finalized `Metadata` account
/// owned by the Program Metadata Program — never true for an address that
/// merely has *some* account at it (e.g. one funded by a prior `upload`
/// interrupted before `initialize`/`set_data` finished). Callers use this to
/// choose between the create and update paths.
fn is_initialized_metadata(client: &RpcClient, address: &Address) -> Result<bool, String> {
    Ok(match fetch_account(client, address)? {
        None => false,
        Some(account) => {
            account.owner == program_metadata::PROGRAM_METADATA_ID
                && matches!(
                    header::parse_header(&account.data).map(|h| h.discriminator),
                    Some(header::AccountDiscriminator::Metadata)
                )
        }
    })
}

/// Clears `target` for a fresh `allocate` when it holds a leftover `Buffer`
/// from an upload interrupted after `allocate`/`write` but before `close`.
/// Does nothing if `target` doesn't exist or isn't owned by the Program
/// Metadata Program. Errors, without closing, if `target` is a finalized
/// `Metadata` account or has an unrecognized header — closing either could
/// destroy real data.
fn reclaim_stray_buffer(session: &Session, target: &Address) -> Outcome {
    let Some(account) = fetch_account(&session.client, target)? else {
        return Ok(());
    };
    if account.owner != program_metadata::PROGRAM_METADATA_ID {
        return Ok(());
    }
    match header::parse_header(&account.data).map(|h| h.discriminator) {
        Some(header::AccountDiscriminator::Buffer) => {
            let close_ix = program_metadata::close(
                target,
                &session.authority(),
                &session.program_id,
                &session.program_data,
                &session.authority(),
            );
            send(
                session,
                close_ix,
                "Close (reclaim leftover buffer from interrupted upload)",
            )
        }
        Some(header::AccountDiscriminator::Metadata) => Err(format!(
            "{target} is unexpectedly a finalized metadata account where a buffer was expected — \
             aborting to avoid data loss."
        )),
        Some(header::AccountDiscriminator::Empty) => Ok(()),
        None => Err(format!(
            "{target} is owned by the Program Metadata Program but its header is not recognized — \
             aborting to avoid data loss."
        )),
    }
}

/// Tops up `target`'s lamport balance to at least the rent-exempt minimum
/// for `size` bytes. Required before `allocate`/`initialize`/`set_data`:
/// none of them transfer lamports themselves, and the runtime enforces
/// rent-exemption on any resize.
fn ensure_rent_exempt(session: &Session, target: &Address, size: usize) -> Outcome {
    let required = retry_read("fetch the rent-exemption amount", || {
        session
            .client
            .get_minimum_balance_for_rent_exemption(size)
            .map_err(|e| format!("Failed to fetch the rent-exemption amount: {e}"))
    })?;
    let current = retry_read(&format!("read the balance of {target}"), || {
        session
            .client
            .get_balance(target)
            .map_err(|e| format!("Failed to read the balance of {target}: {e}"))
    })?;
    if current >= required {
        return Ok(());
    }
    let ix = solana_system_interface::instruction::transfer(
        &session.authority(),
        target,
        required - current,
    );
    send(session, ix, "Fund account for rent-exemption")
}

/// Signs and sends `ix`, returning once it is confirmed. A failure that is
/// not a definitive rejection is retried up to [`MAX_ATTEMPTS`] times in
/// total, but a transaction is only ever rebuilt with a new blockhash after
/// its old blockhash has expired and its signature is confirmed absent, so
/// one call can never apply `ix` twice.
fn send(session: &Session, ix: Instruction, label: &str) -> Outcome {
    let client = &session.client;
    let mut failures = 0u32;
    loop {
        let (blockhash, _) = retry_read("fetch the latest blockhash", || {
            client
                .get_latest_blockhash_with_commitment(client.commitment())
                .map_err(|e| format!("Failed to fetch the latest blockhash: {e}"))
        })?;
        let tx = Transaction::new_signed_with_payer(
            std::slice::from_ref(&ix),
            Some(&session.authority()),
            &[&session.payer],
            blockhash,
        );
        let signature = tx.signatures[0];

        loop {
            let error = match client.send_and_confirm_transaction(&tx) {
                Ok(signature) => {
                    ui::success(format!("{label} — {signature}"));
                    return Ok(());
                }
                Err(error) => error,
            };

            let expired = !retry_read("check the blockhash", || {
                client
                    .is_blockhash_valid(&blockhash, client.commitment())
                    .map_err(|e| format!("Failed to check the blockhash: {e}"))
            })?;
            let status = retry_read("check the transaction status", || {
                client
                    .get_signature_status(&signature)
                    .map_err(|e| format!("Failed to check the transaction status: {e}"))
            })?;
            match status {
                Some(Ok(())) => {
                    ui::success(format!("{label} — {signature}"));
                    return Ok(());
                }
                Some(Err(rejection)) => {
                    return Err(format!("{label} failed on chain: {rejection}"))
                }
                None => {}
            }
            if !expired && error.get_transaction_error().is_some() {
                return Err(format!("{label} failed: {error}"));
            }

            failures += 1;
            if failures >= MAX_ATTEMPTS {
                return Err(format!(
                    "{label} failed after {MAX_ATTEMPTS} attempts: {error}. Run the command \
                     again to continue where it stopped."
                ));
            }
            ui::warn(format!(
                "{label}: {error} — retrying ({failures}/{})",
                MAX_ATTEMPTS - 1
            ));
            std::thread::sleep(backoff(Duration::from_secs(1), failures));
            if expired {
                break;
            }
        }
    }
}

fn write_chunks(session: &Session, buffer: &Address, data: &[u8]) -> Outcome {
    let mut offset = 0u32;
    let total = data.len();
    while (offset as usize) < total {
        let end = std::cmp::min(offset as usize + CHUNK_SIZE, total);
        let chunk = &data[offset as usize..end];
        let ix = program_metadata::write(buffer, &session.authority(), offset, chunk);
        send(session, ix, &format!("Write ({end}/{total} bytes)"))?;
        offset = end as u32;
    }
    Ok(())
}

fn create_new(session: &Session, seed: &Seed, compressed: &[u8]) -> Outcome {
    let authority = session.authority();
    let accounts = || MetadataAccounts {
        metadata: &session.metadata_pda,
        authority: &authority,
        program: &session.program_id,
        program_data: &session.program_data,
    };
    let final_size = HEADER_LEN + compressed.len();

    if compressed.len() <= CHUNK_SIZE {
        ensure_rent_exempt(session, &session.metadata_pda, final_size)?;
        let ix = program_metadata::initialize(
            accounts(),
            seed,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
            Some(compressed),
        );
        send(session, ix, "Initialize (inline)")?;
        ui::success(format!("IDL uploaded ({})", session.metadata_pda));
        return Ok(());
    }

    reclaim_stray_buffer(session, &session.metadata_pda)?;
    ensure_rent_exempt(session, &session.metadata_pda, final_size)?;
    let allocate_ix = program_metadata::allocate(
        &session.metadata_pda,
        &authority,
        &session.program_id,
        &session.program_data,
        seed,
    );
    send(session, allocate_ix, "Allocate")?;
    write_chunks(session, &session.metadata_pda, compressed)?;
    let init_ix = program_metadata::initialize(
        accounts(),
        seed,
        encoding::UTF8,
        compression::ZLIB,
        format::JSON,
        data_source::DIRECT,
        None,
    );
    send(session, init_ix, "Initialize (finalize buffer)")?;
    ui::success(format!("IDL uploaded ({})", session.metadata_pda));
    Ok(())
}

fn update_existing(session: &Session, compressed: &[u8]) -> Outcome {
    let authority = session.authority();
    let accounts = || MetadataAccounts {
        metadata: &session.metadata_pda,
        authority: &authority,
        program: &session.program_id,
        program_data: &session.program_data,
    };
    let final_size = HEADER_LEN + compressed.len();

    ensure_rent_exempt(session, &session.metadata_pda, final_size)?;

    if compressed.len() <= CHUNK_SIZE {
        let ix = program_metadata::set_data(
            accounts(),
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            SetDataSource::Inline {
                data_source: data_source::DIRECT,
                data: compressed,
            },
        );
        send(session, ix, "SetData (inline)")?;
        ui::success(format!("IDL updated ({})", session.metadata_pda));
        return Ok(());
    }

    let buffer_seed = program_metadata::seed_from_str("idl-buffer");
    let (buffer_pda, _bump) =
        program_metadata::derive_canonical_metadata_pda(&session.program_id, &buffer_seed);

    reclaim_stray_buffer(session, &buffer_pda)?;
    ensure_rent_exempt(session, &buffer_pda, final_size)?;
    let allocate_ix = program_metadata::allocate(
        &buffer_pda,
        &authority,
        &session.program_id,
        &session.program_data,
        &buffer_seed,
    );
    send(session, allocate_ix, "Allocate (temp buffer)")?;
    write_chunks(session, &buffer_pda, compressed)?;

    let set_data_ix = program_metadata::set_data(
        accounts(),
        encoding::UTF8,
        compression::ZLIB,
        format::JSON,
        SetDataSource::FromBuffer {
            data_source: data_source::DIRECT,
            buffer: buffer_pda,
        },
    );
    send(session, set_data_ix, "SetData (from buffer)")?;

    let close_ix = program_metadata::close(
        &buffer_pda,
        &authority,
        &session.program_id,
        &session.program_data,
        &authority,
    );
    send(session, close_ix, "Close (temp buffer cleanup)")?;
    ui::success(format!("IDL updated ({})", session.metadata_pda));
    Ok(())
}

pub fn execute_upload(program_id_str: &str) -> Outcome {
    let session = Session::open(program_id_str)?;
    let root = workspace::root();

    let program_name =
        find_program_name(&root, program_id_str, &session.cluster).ok_or_else(|| {
            format!(
                "Could not find program '{program_id_str}' in Naclac.toml under [programs.{}].",
                session.cluster
            )
        })?;

    let idl_path = naclac_client_gen::idl_dir(&root).join(format!("{program_name}.json"));
    let idl_content = fs::read_to_string(&idl_path).map_err(|_| {
        format!(
            "No compiled IDL found at {} — run `naclac build` first.",
            idl_path.display()
        )
    })?;
    let idl: naclac_idl::Idl = serde_json::from_str(&idl_content).map_err(|e| {
        format!(
            "Failed to parse {} as a naclac IDL: {e}",
            idl_path.display()
        )
    })?;
    let onchain_idl = naclac_idl::converter::to_onchain_idl(&idl)?;
    let onchain_idl_json = serde_json::to_string(&onchain_idl)
        .map_err(|e| format!("Failed to serialize the on-chain IDL: {e}"))?;

    let mut encoder = ZlibEncoder::new(Vec::new(), ZlibCompression::default());
    encoder
        .write_all(onchain_idl_json.as_bytes())
        .map_err(|e| format!("Failed to compress the IDL: {e}"))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("Failed to compress the IDL: {e}"))?;

    require_authorized(&session)?;

    if is_initialized_metadata(&session.client, &session.metadata_pda)? {
        update_existing(&session, &compressed)
    } else {
        let seed = program_metadata::seed_from_str("idl");
        create_new(&session, &seed, &compressed)
    }
}

pub fn execute_set_immutable(program_id_str: &str, yes: bool) -> Outcome {
    let session = Session::open(program_id_str)?;
    require_authorized(&session)?;
    confirm_irreversible(
        &format!(
            "Permanently lock the IDL metadata account {} of {program_id_str}? This cannot be undone.",
            session.metadata_pda
        ),
        yes,
    )?;

    let ix = program_metadata::set_immutable(
        &session.metadata_pda,
        &session.authority(),
        &session.program_id,
        &session.program_data,
    );
    send(&session, ix, "SetImmutable")?;
    ui::success(format!(
        "IDL metadata is now permanently immutable ({})",
        session.metadata_pda
    ));
    Ok(())
}

/// The address a close/trim refunds to: `destination` when given, otherwise
/// the signing wallet (the documented default of `--destination`).
fn refund_destination(destination: Option<&str>, wallet: Address) -> Result<Address, String> {
    match destination {
        Some(d) => parse_address("destination", d),
        None => Ok(wallet),
    }
}

pub fn execute_close(program_id_str: &str, destination: Option<&str>, yes: bool) -> Outcome {
    let session = Session::open(program_id_str)?;
    let destination_addr = refund_destination(destination, session.authority())?;
    require_authorized(&session)?;
    confirm_irreversible(
        &format!(
            "Permanently delete the IDL metadata account {} of {program_id_str}, refunding its rent to {destination_addr}?",
            session.metadata_pda
        ),
        yes,
    )?;

    let ix = program_metadata::close(
        &session.metadata_pda,
        &session.authority(),
        &session.program_id,
        &session.program_data,
        &destination_addr,
    );
    send(&session, ix, "Close")?;
    ui::success(format!(
        "IDL metadata account closed ({})",
        session.metadata_pda
    ));
    Ok(())
}

pub fn execute_trim(program_id_str: &str, destination: Option<&str>) -> Outcome {
    let session = Session::open(program_id_str)?;
    let destination_addr = refund_destination(destination, session.authority())?;
    require_authorized(&session)?;

    let ix = program_metadata::trim(
        &session.metadata_pda,
        &session.authority(),
        &session.program_id,
        &session.program_data,
        &destination_addr,
    );
    send(&session, ix, "Trim")?;
    ui::success(format!(
        "IDL metadata account trimmed ({})",
        session.metadata_pda
    ));
    Ok(())
}

pub fn execute_set_authority(
    program_id_str: &str,
    new_authority: Option<&str>,
    remove: bool,
) -> Outcome {
    if remove && new_authority.is_some() {
        return Err("Pass either --new-authority or --remove, not both.".to_string());
    }
    let new_authority_addr =
        match new_authority {
            Some(a) => Some(parse_address("new-authority", a)?),
            None if remove => None,
            None => return Err(
                "Pass --new-authority <PUBKEY> to set a new authority, or --remove to remove it."
                    .to_string(),
            ),
        };

    let session = Session::open(program_id_str)?;
    require_authorized(&session)?;

    let label = match &new_authority_addr {
        Some(a) => format!("SetAuthority (-> {a})"),
        None => "SetAuthority (remove)".to_string(),
    };
    let ix = program_metadata::set_authority(
        &session.metadata_pda,
        &session.authority(),
        &session.program_id,
        &session.program_data,
        new_authority_addr.as_ref(),
    );
    send(&session, ix, &label)?;
    ui::success("Authority updated.");
    Ok(())
}

/// The payload decompressed per `compression_byte`, never larger than
/// [`MAX_IDL_BYTES`].
fn decompress(compression_byte: u8, raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let read = match compression_byte {
        compression::NONE => return Ok(raw.to_vec()),
        compression::GZIP => GzDecoder::new(raw)
            .take(MAX_IDL_BYTES + 1)
            .read_to_end(&mut out),
        compression::ZLIB => ZlibDecoder::new(raw)
            .take(MAX_IDL_BYTES + 1)
            .read_to_end(&mut out),
        other => {
            return Err(format!(
                "Unrecognized compression byte {other} on the metadata account."
            ))
        }
    };
    read.map_err(|e| format!("Failed to decompress the IDL payload: {e}"))?;
    if out.len() as u64 > MAX_IDL_BYTES {
        return Err(format!(
            "The IDL payload decompresses to more than {MAX_IDL_BYTES} bytes."
        ));
    }
    Ok(out)
}

/// Writes the IDL uploaded for `program_id_str` to `<dir>/<file_name>.json`
/// (`file_name` defaults to the program's name in Naclac.toml, `dir` to
/// `artifacts/onchain-idl`) and never overwrites a file; with `stdout` it
/// prints the IDL instead. A naclac IDL is restored to its local form, unless
/// `raw` keeps the on-chain form, which then defaults to the file name
/// `<name>.onchain.json`. Errors unless the account is a finalized, directly
/// stored UTF-8 JSON IDL.
pub fn execute_fetch(
    program_id_str: &str,
    program_name: Option<&str>,
    file_name: Option<&str>,
    dir: Option<&str>,
    stdout: bool,
    raw: bool,
) -> Outcome {
    let config = load_naclac_config();
    let program_id = parse_address("program", program_id_str)?;

    let seed = program_metadata::seed_from_str("idl");
    let (metadata_pda, _bump) = program_metadata::derive_canonical_metadata_pda(&program_id, &seed);
    let client = RpcClient::new(config.rpc_url.clone());
    require_programs_loaded(&client, &config.cluster, &program_id)?;

    let account = fetch_account(&client, &metadata_pda)?.ok_or_else(|| {
        format!(
            "No metadata account found at {metadata_pda} — has this program's IDL been uploaded?"
        )
    })?;
    if account.owner != program_metadata::PROGRAM_METADATA_ID {
        return Err(format!(
            "{metadata_pda} is not owned by the Program Metadata Program (owner: {}).",
            account.owner
        ));
    }
    let parsed = header::parse_header(&account.data)
        .ok_or_else(|| format!("Metadata account {metadata_pda} has an unrecognized layout."))?;

    match parsed.discriminator {
        header::AccountDiscriminator::Metadata => {}
        header::AccountDiscriminator::Buffer => {
            return Err(format!(
                "{metadata_pda} holds an unfinished upload buffer, not an IDL — run `naclac idl \
                 upload` again to finish the upload."
            ))
        }
        header::AccountDiscriminator::Empty => {
            return Err(format!(
                "{metadata_pda} is an empty metadata account, not an IDL."
            ))
        }
    }
    if parsed.data_source != data_source::DIRECT {
        return Err(format!(
            "The IDL at {metadata_pda} is stored by reference (data source {}), which `naclac \
             idl fetch` does not follow.",
            parsed.data_source
        ));
    }
    if parsed.encoding != encoding::UTF8 || parsed.format != format::JSON {
        return Err(format!(
            "The IDL at {metadata_pda} uses encoding {} and format {}; `naclac idl fetch` \
             supports only UTF-8 JSON.",
            parsed.encoding, parsed.format
        ));
    }

    let raw_payload = header::payload(&account.data, &parsed)
        .ok_or_else(|| format!("Metadata account {metadata_pda} is truncated or corrupt."))?;
    let idl_text = String::from_utf8(decompress(parsed.compression, raw_payload)?)
        .map_err(|_| format!("The IDL at {metadata_pda} is not valid UTF-8."))?;
    let idl_value: serde_json::Value = serde_json::from_str(&idl_text)
        .map_err(|e| format!("The IDL at {metadata_pda} is not valid JSON: {e}"))?;
    let idl_value = if raw {
        idl_value
    } else if naclac_idl::converter::is_naclac_idl(&idl_value) {
        naclac_idl::converter::to_local_idl(idl_value)
            .map_err(|e| format!("The IDL at {metadata_pda} cannot be restored: {e}"))?
    } else {
        ui::warn(format!(
            "The IDL at {metadata_pda} was not uploaded by naclac; it is written as stored."
        ));
        idl_value
    };
    let idl_text = naclac_idl::to_compact_pretty_json(&idl_value)
        .map_err(|e| format!("Failed to format the IDL at {metadata_pda}: {e}"))?;

    if stdout {
        println!("{idl_text}");
        return Ok(());
    }

    let file_name = match (file_name, program_name) {
        (Some(file_name), _) => file_name.to_string(),
        (None, Some(program_name)) if raw => format!("{program_name}.onchain"),
        (None, Some(program_name)) => program_name.to_string(),
        (None, None) => {
            return Err(format!(
                "{program_id_str} is not listed in `[programs.{}]`, so it has no default file \
                 name. Choose one with --name.",
                config.cluster
            ))
        }
    };
    if file_name.contains('/') || file_name.contains('\\') {
        return Err(
            "`--name` is a file name, not a path; choose the folder with --dir.".to_string(),
        );
    }

    let dir = match dir {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            match fs::metadata(&dir) {
                Ok(metadata) if metadata.is_dir() => dir,
                Ok(_) => return Err(format!("`--dir` {} is not a folder.", dir.display())),
                Err(e) => return Err(format!("Cannot use `--dir` {}: {e}", dir.display())),
            }
        }
        None => {
            let dir = naclac_client_gen::artifacts_dir(&workspace::root()).join(DEFAULT_FETCH_DIR);
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
            dir
        }
    };

    let path = dir.join(format!("{file_name}.json"));
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "{} already exists. Choose another name with --name, or another folder with --dir.",
                path.display()
            ))
        }
        Err(e) => return Err(format!("Cannot create {}: {e}", path.display())),
    };
    file.write_all(idl_text.as_bytes())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    ui::success(format!("IDL written to {}", path.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program_data_account(tag: u32, option: u8, authority: [u8; 32]) -> OnChainAccount {
        let mut data = Vec::new();
        data.extend_from_slice(&tag.to_le_bytes());
        data.extend_from_slice(&7u64.to_le_bytes());
        data.push(option);
        data.extend_from_slice(&authority);
        data.extend_from_slice(b"program bytes follow");
        OnChainAccount {
            owner: program_metadata::BPF_LOADER_UPGRADEABLE_ID,
            data,
            executable: false,
        }
    }

    fn address() -> Address {
        Address::new_from_array([9u8; 32])
    }

    #[test]
    fn upgrade_authority_is_read_when_present() {
        let account = program_data_account(3, 1, [5u8; 32]);
        assert_eq!(
            parse_upgrade_authority(&account, &address()),
            Ok(Some(Address::new_from_array([5u8; 32])))
        );
    }

    #[test]
    fn immutable_program_has_no_upgrade_authority() {
        let account = program_data_account(3, 0, [0u8; 32]);
        assert_eq!(parse_upgrade_authority(&account, &address()), Ok(None));
    }

    #[test]
    fn wrong_variant_tag_is_rejected() {
        let account = program_data_account(2, 1, [5u8; 32]);
        assert!(parse_upgrade_authority(&account, &address()).is_err());
    }

    #[test]
    fn invalid_option_tag_is_rejected() {
        let account = program_data_account(3, 2, [5u8; 32]);
        assert!(parse_upgrade_authority(&account, &address()).is_err());
    }

    #[test]
    fn foreign_owner_is_rejected() {
        let mut account = program_data_account(3, 1, [5u8; 32]);
        account.owner = Address::new_from_array([1u8; 32]);
        assert!(parse_upgrade_authority(&account, &address()).is_err());
    }

    #[test]
    fn truncated_program_data_is_rejected() {
        let mut account = program_data_account(3, 1, [5u8; 32]);
        account.data.truncate(PROGRAM_DATA_METADATA_LEN - 1);
        assert!(parse_upgrade_authority(&account, &address()).is_err());
    }

    #[test]
    fn decompress_rejects_oversized_output() {
        let mut encoder = ZlibEncoder::new(Vec::new(), ZlibCompression::default());
        encoder
            .write_all(&vec![b'a'; MAX_IDL_BYTES as usize + 1])
            .unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(decompress(compression::ZLIB, &compressed).is_err());
    }

    #[test]
    fn decompress_round_trips_zlib() {
        let mut encoder = ZlibEncoder::new(Vec::new(), ZlibCompression::default());
        encoder.write_all(b"{\"a\":1}").unwrap();
        let compressed = encoder.finish().unwrap();
        assert_eq!(
            decompress(compression::ZLIB, &compressed),
            Ok(b"{\"a\":1}".to_vec())
        );
    }

    #[test]
    fn a_read_that_succeeds_first_time_is_not_repeated() {
        let mut calls = 0;
        let result = retry_read_with(Duration::from_millis(1), "read", || {
            calls += 1;
            Ok::<_, String>(7)
        });
        assert_eq!(result, Ok(7));
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_read_that_fails_twice_then_succeeds_returns_the_value() {
        let mut calls = 0;
        let result = retry_read_with(Duration::from_millis(1), "read", || {
            calls += 1;
            if calls < 3 {
                Err("network".to_string())
            } else {
                Ok(calls)
            }
        });
        assert_eq!(result, Ok(3));
    }

    #[test]
    fn a_read_that_never_succeeds_gives_up_after_max_attempts() {
        let mut calls = 0;
        let result: Result<(), String> = retry_read_with(Duration::from_millis(1), "read", || {
            calls += 1;
            Err("down".to_string())
        });
        assert_eq!(calls, MAX_ATTEMPTS);
        let message = result.unwrap_err();
        assert!(message.contains("down"));
        assert!(message.contains("gave up after 5 attempts"));
    }

    #[test]
    fn backoff_doubles_each_failed_attempt() {
        let base = Duration::from_secs(1);
        assert_eq!(backoff(base, 1), Duration::from_secs(1));
        assert_eq!(backoff(base, 2), Duration::from_secs(2));
        assert_eq!(backoff(base, 3), Duration::from_secs(4));
        assert_eq!(backoff(base, 4), Duration::from_secs(8));
    }

    #[test]
    fn decompress_rejects_unknown_compression() {
        assert!(decompress(9, b"x").is_err());
    }
}
