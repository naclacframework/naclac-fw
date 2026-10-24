use {
    crate::error::{translate_error_code, NaclacClientError},
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_commitment_config::CommitmentConfig,
    solana_hash::Hash,
    solana_instruction_error::InstructionError,
    solana_keypair::Keypair,
    solana_rpc_client::rpc_client::RpcClient,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, TransactionError},
    std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    },
};

#[derive(Debug, Clone)]
pub struct NaclacTransactionMetadata {
    pub signature: Signature,
    pub logs: Vec<String>,
    pub compute_units_consumed: u64,
    /// The instruction's `set_return_data` payload, if any — populated on
    /// every backend (litesvm, and RPC against localnet/devnet/mainnet
    /// alike), not just litesvm. Empty when the instruction never called
    /// `set_return_data`.
    pub return_data: Vec<u8>,
    /// Every instruction invoked via CPI during the transaction, in
    /// execution order, with account keys already resolved. `None` when the
    /// backend could not supply them (as opposed to `Some(vec![])`, which
    /// means the transaction genuinely made no CPIs). Accounts loaded through
    /// an address lookup table cannot be resolved and yield `None` too.
    pub inner_instructions: Option<Vec<NaclacInnerInstruction>>,
}

/// One CPI-invoked instruction from a transaction's inner-instruction trace.
#[derive(Debug, Clone)]
pub struct NaclacInnerInstruction {
    /// Index of the top-level instruction this CPI was made under.
    pub outer_index: usize,
    /// 2 for a CPI made directly by a top-level instruction, 3 for a CPI
    /// made by that CPI, and so on. 0 when an RPC node didn't report it.
    pub stack_height: u8,
    pub program_id: solana_address::Address,
    pub accounts: Vec<solana_address::Address>,
    pub data: Vec<u8>,
}

/// Resolves litesvm's compiled inner instructions against the transaction's
/// static account keys. `None` if any index falls outside them (i.e. the
/// transaction used an address lookup table).
fn resolve_litesvm_inner_instructions(
    keys: &[Address],
    list: &solana_message::inner_instruction::InnerInstructionsList,
) -> Option<Vec<NaclacInnerInstruction>> {
    let key_at = |index: usize| {
        keys.get(index)
            .map(|key| solana_address::Address::new_from_array(key.to_bytes()))
    };
    let mut resolved = Vec::new();
    for (outer_index, set) in list.iter().enumerate() {
        for inner in set {
            resolved.push(NaclacInnerInstruction {
                outer_index,
                stack_height: inner.stack_height,
                program_id: key_at(inner.instruction.program_id_index as usize)?,
                accounts: inner
                    .instruction
                    .accounts
                    .iter()
                    .map(|index| key_at(*index as usize))
                    .collect::<Option<Vec<_>>>()?,
                data: inner.instruction.data.clone(),
            });
        }
    }
    Some(resolved)
}

/// Resolves RPC-reported compiled inner instructions against the
/// transaction's full account-key list (static keys, then ALT-loaded writable
/// keys, then ALT-loaded readonly keys). `None` if an instruction isn't in
/// compiled form or refers to a key outside `keys`. A node that doesn't
/// report `stackHeight` yields `stack_height: 0`.
fn resolve_ui_inner_instructions(
    keys: &[solana_address::Address],
    list: &[solana_transaction_status::UiInnerInstructions],
) -> Option<Vec<NaclacInnerInstruction>> {
    let mut resolved = Vec::new();
    for set in list {
        for ui in &set.instructions {
            let solana_transaction_status::UiInstruction::Compiled(compiled) = ui else {
                return None;
            };
            resolved.push(NaclacInnerInstruction {
                outer_index: set.index as usize,
                stack_height: compiled.stack_height.map_or(0, |height| height as u8),
                program_id: *keys.get(compiled.program_id_index as usize)?,
                accounts: compiled
                    .accounts
                    .iter()
                    .map(|index| keys.get(*index as usize).copied())
                    .collect::<Option<Vec<_>>>()?,
                data: bs58::decode(&compiled.data).into_vec().ok()?,
            });
        }
    }
    Some(resolved)
}

/// Inner instructions of a `getTransaction` response fetched with binary
/// encoding. `None` when the node omitted them, the transaction can't be
/// decoded, or any key can't be resolved.
fn inner_instructions_from_rpc(
    encoded: &solana_transaction_status::EncodedTransactionWithStatusMeta,
) -> Option<Vec<NaclacInnerInstruction>> {
    use solana_transaction_status::option_serializer::OptionSerializer;
    let meta = encoded.meta.as_ref()?;
    // A failed transaction's recorded inner instructions were never
    // committed, so none of them can vouch for an event.
    if meta.err.is_some() {
        return None;
    }
    let OptionSerializer::Some(list) = &meta.inner_instructions else {
        return None;
    };
    let tx = encoded.transaction.decode()?;
    let mut keys: Vec<solana_address::Address> = tx
        .message
        .static_account_keys()
        .iter()
        .map(|key| solana_address::Address::new_from_array(key.to_bytes()))
        .collect();
    if let OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        for encoded_key in loaded.writable.iter().chain(loaded.readonly.iter()) {
            let bytes: [u8; 32] = bs58::decode(encoded_key).into_vec().ok()?.try_into().ok()?;
            keys.push(solana_address::Address::new_from_array(bytes));
        }
    }
    resolve_ui_inner_instructions(&keys, list)
}

/// Splits one `Program data:` log line into `(discriminator, payload)`. A
/// single base64 chunk carries both concatenated, split at `disc_len`; two or
/// more chunks are `[discriminator, payload]` as emitted by the zero-copy path.
fn extract_event_bytes(parts: &[&str], disc_len: usize) -> Option<(Vec<u8>, Vec<u8>)> {
    use base64::Engine;
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(parts[0])
            .ok()?;
        if bytes.len() >= disc_len {
            let disc = bytes[..disc_len].to_vec();
            let payload = bytes[disc_len..].to_vec();
            Some((disc, payload))
        } else {
            None
        }
    } else {
        let disc = base64::engine::general_purpose::STANDARD
            .decode(parts[0])
            .ok()?;
        let payload = base64::engine::general_purpose::STANDARD
            .decode(parts[1])
            .ok()?;
        Some((disc, payload))
    }
}

impl NaclacTransactionMetadata {
    /// Every `(discriminator, payload)` pair in this transaction that could
    /// be an event of `T`'s program: first those `emit()` wrote to the logs
    /// (in log order), then those `emit_cpi!` sent as self-CPIs (in execution
    /// order). A CPI-emitted event is accepted only from an invocation of
    /// `T::PROGRAM_ID` whose first account is that program's own
    /// `event_authority` PDA and whose data starts with `T::CPI_TAG` — the
    /// program's receive arm rejects the call unless that PDA actually signed.
    fn event_frames<T: NaclacEvent>(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        let disc_len = T::DISCRIMINATOR.len();
        let mut frames = Vec::new();

        for log in &self.logs {
            if let Some(data_str) = log.strip_prefix("Program data: ") {
                let parts: Vec<&str> = data_str.split_whitespace().collect();
                if let Some(frame) = extract_event_bytes(&parts, disc_len) {
                    frames.push(frame);
                }
            }
        }

        let tag = T::CPI_TAG;
        if let (false, Some(inner)) = (tag.is_empty(), &self.inner_instructions) {
            let (event_authority, _bump) = solana_address::Address::find_program_address(
                &[b"__event_authority"],
                &T::PROGRAM_ID,
            );
            for call in inner {
                if call.program_id == T::PROGRAM_ID
                    && call.accounts.first() == Some(&event_authority)
                    && call.data.starts_with(tag)
                    && call.data.len() >= tag.len() + disc_len
                {
                    let body = &call.data[tag.len()..];
                    frames.push((body[..disc_len].to_vec(), body[disc_len..].to_vec()));
                }
            }
        }
        frames
    }

    /// Parses Borsh-serialized events of type `T` from the transaction's
    /// logs and, for a program using `#[event_cpi]`, its inner instructions.
    pub fn parse_events_borsh<T: borsh::BorshDeserialize + NaclacEvent>(
        &self,
    ) -> Result<Vec<T>, NaclacClientError> {
        let mut events = Vec::new();
        for (disc_bytes, payload_bytes) in self.event_frames::<T>() {
            if disc_bytes == T::DISCRIMINATOR {
                let mut reader = &payload_bytes[..];
                if let Ok(event) = T::deserialize(&mut reader) {
                    events.push(event);
                }
            }
        }
        Ok(events)
    }

    /// Parses Wincode-serialized events of type `T` from the transaction's
    /// logs and, for a program using `#[event_cpi]`, its inner instructions.
    pub fn parse_events_wincode<
        T: wincode::SchemaReadOwned<crate::wincode_config::Config, Dst = T> + NaclacEvent,
    >(
        &self,
    ) -> Result<Vec<T>, NaclacClientError> {
        let mut events = Vec::new();
        for (disc_bytes, payload_bytes) in self.event_frames::<T>() {
            if disc_bytes == T::DISCRIMINATOR {
                let mut reader = &payload_bytes[..];
                if let Ok(event) =
                    wincode::config::deserialize_from(&mut reader, crate::wincode_config::CONFIG)
                {
                    events.push(event);
                }
            }
        }
        Ok(events)
    }

    /// Parses Zero-Copy (bytemuck Pod) events of type `T` from the
    /// transaction's logs and, for a program using `#[event_cpi]`, its inner
    /// instructions.
    pub fn parse_events_zero_copy<T: bytemuck::Pod + NaclacEvent>(
        &self,
    ) -> Result<Vec<T>, NaclacClientError> {
        let mut events = Vec::new();
        for (disc_bytes, payload_bytes) in self.event_frames::<T>() {
            if disc_bytes == T::DISCRIMINATOR && payload_bytes.len() >= std::mem::size_of::<T>() {
                events.push(bytemuck::pod_read_unaligned(
                    &payload_bytes[0..std::mem::size_of::<T>()],
                ));
            }
        }
        Ok(events)
    }

    /// Parses `#[event(alloc)]` events of type `T` from the transaction's
    /// logs and, for a program using `#[event_cpi]`, its inner instructions.
    /// Unlike [`parse_events_zero_copy`](Self::parse_events_zero_copy), `T` isn't
    /// `bytemuck::Pod` (it may hold `Vec`/`String`/`Option` fields), so decoding
    /// is delegated to `T`'s generated [`NaclacAllocEvent::decode`], which mirrors
    /// the on-chain `emit()`'s sequential, length-prefixed write order.
    pub fn parse_events_alloc<T: crate::NaclacAllocEvent>(
        &self,
    ) -> Result<Vec<T>, NaclacClientError> {
        let mut events = Vec::new();
        for (disc_bytes, payload_bytes) in self.event_frames::<T>() {
            if disc_bytes == T::DISCRIMINATOR {
                if let Some(event) = T::decode(&payload_bytes) {
                    events.push(event);
                }
            }
        }
        Ok(events)
    }
}

/// Implemented by client-generated `#[event(alloc)]` structs. `decode` reads
/// the single-allocation, length-prefixed wire format `expand_alloc` writes
/// on-chain: fields in declaration order, `Vec<T>`/`String` as a u32 LE
/// length prefix followed by their bytes, `Option<T>` as a 1-byte tag plus
/// `T`'s bytes when present, everything else as a raw `bytemuck` read.
pub trait NaclacAllocEvent: NaclacEvent + Sized {
    fn decode(bytes: &[u8]) -> Option<Self>;
}

/// Implemented by every client-generated event struct. `DISCRIMINATOR` is the
/// value from the program's IDL (the name-derived default or the program's
/// custom override): the bytes that prefix this event in a transaction log.
pub trait NaclacEvent {
    const DISCRIMINATOR: &'static [u8];
    /// The program that emits this event. Only self-CPI invocations of this
    /// program are decoded from a transaction's inner instructions.
    const PROGRAM_ID: solana_address::Address;
    /// The program's reserved self-CPI tag from its IDL (`eventCpiTag`);
    /// empty when the program never uses `#[event_cpi]`, which turns
    /// inner-instruction decoding off for this event.
    const CPI_TAG: &'static [u8];
}

#[derive(Clone)]
pub enum ClientBackend {
    LiteSVM(Arc<Mutex<LiteSVM>>),
    Rpc(Arc<RpcClient>),
}

/// Which real cluster an RPC-backed `NaclacProvider` is talking to — inferred
/// from the URL (matching the same known-endpoint strings `NaclacProvider::new`
/// already dispatches on), since only `ClientBackend::Rpc` carries a bare
/// `RpcClient` with no cluster identity of its own. Used by `airdrop` to
/// decide whether to cap the requested amount — real cluster faucets differ
/// wildly in how rate-limited they are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RpcCluster {
    Litesvm,
    Localnet,
    Devnet,
    Testnet,
    Mainnet,
    Custom,
}

/// Walks up from the current directory looking for `Naclac.toml`, mirroring
/// `naclac deploy`'s own workspace-root discovery (`cli/src/commands/deploy.rs`)
/// so both land on the same directory. Returns `None` outside a Naclac
/// workspace — that's not an error, just nothing to auto-load.
fn find_workspace_root() -> Option<std::path::PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    if current_dir.join("Naclac.toml").exists() {
        Some(current_dir)
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().ok()
    } else {
        None
    }
}

/// Resolves a `.so`'s program ID from its co-located `<name>-keypair.json`,
/// using the exact same derivation Agave's `solana program deploy` uses
/// (`get_default_program_keypair` in `agave/cli/src/program.rs`): the file
/// stem (not a naive `.so`-suffix replace) with `-keypair.json` appended.
/// Unlike Agave's own fallback-to-a-random-keypair behavior for a first-time
/// deploy, a missing/unparsable keypair file here is a real error — there's
/// no way to know what ID a caller's `declare_id!`/generated client expects
/// otherwise, and inventing one would silently fail to match either.
fn resolve_program_id_from_so(so_path: &std::path::Path) -> Result<Address, NaclacClientError> {
    let stem = so_path.file_stem().ok_or_else(|| {
        NaclacClientError::General(format!(
            "Cannot determine program name from '{}'",
            so_path.display()
        ))
    })?;
    let mut keypair_path = so_path.to_path_buf();
    let mut filename = stem.to_os_string();
    filename.push("-keypair");
    keypair_path.set_file_name(filename);
    keypair_path.set_extension("json");

    solana_keypair::read_keypair_file(&keypair_path)
        .map(|kp| kp.pubkey())
        .map_err(|e| {
            NaclacClientError::General(format!(
                "Failed to resolve program ID for '{}': could not read keypair file '{}': {}",
                so_path.display(),
                keypair_path.display(),
                e
            ))
        })
}

/// Auto-loads every `.so` in `artifacts/deploy/` into a fresh litesvm instance,
/// so `NaclacProvider::new("litesvm", payer)` is usable the same way
/// `"localnet"` already is — no manual `add_program` call needed for the
/// workspace's own program(s). A no-op outside a Naclac workspace or before
/// the first `naclac build`.
fn auto_load_workspace_programs(svm: &mut LiteSVM) -> Result<(), NaclacClientError> {
    let Some(workspace_root) = find_workspace_root() else {
        return Ok(());
    };
    let deploy_dir = crate::utils::workspace_deploy_dir(&workspace_root);
    if !deploy_dir.exists() {
        return Ok(());
    }

    let entries = std::fs::read_dir(&deploy_dir).map_err(|e| {
        NaclacClientError::General(format!("Failed to read '{}': {}", deploy_dir.display(), e))
    })?;

    for entry in entries {
        let entry = entry
            .map_err(|e| NaclacClientError::General(format!("Failed to read dir entry: {}", e)))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("so") {
            continue;
        }

        let program_id = resolve_program_id_from_so(&path)?;
        svm.add_program_from_file(program_id, &path).map_err(|e| {
            NaclacClientError::LiteSvmError(format!(
                "Failed to auto-load program '{}': {:?}",
                path.display(),
                e
            ))
        })?;
    }

    Ok(())
}

/// Extracts a friendly error message from a failed transaction's
/// `TransactionError`, translating a custom program error code via
/// `translate_error_code` when there is one. Shared by `send_transaction`,
/// `simulate_transaction`, and `get_transaction` so the three don't drift
/// out of sync with each other.
fn translate_transaction_failure(
    err: &TransactionError,
    logs: Vec<String>,
    account_names: Option<&[&str]>,
) -> NaclacClientError {
    let (ins_err, err_code) = match err {
        TransactionError::InstructionError(_idx, ix_err) => {
            let code = if let InstructionError::Custom(c) = ix_err {
                Some(*c)
            } else {
                None
            };
            (ix_err.clone(), code)
        }
        _ => (InstructionError::GenericError, None),
    };
    let translated_msg = if let Some(code) = err_code {
        translate_error_code(code, account_names, &logs)
    } else {
        format!("{:?}", err)
    };
    NaclacClientError::TransactionFailed {
        instruction_err: ins_err,
        logs,
        translated_msg,
    }
}

fn cluster_from_url(url: &str) -> RpcCluster {
    if url.contains("devnet") {
        RpcCluster::Devnet
    } else if url.contains("testnet") {
        RpcCluster::Testnet
    } else if url.contains("mainnet") {
        RpcCluster::Mainnet
    } else if url.contains("127.0.0.1") || url.contains("localhost") {
        RpcCluster::Localnet
    } else {
        RpcCluster::Custom
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TransactionVersion {
    #[default]
    V1,
    V0,
    Legacy,
}

#[derive(Clone)]
pub struct NaclacProvider {
    pub backend: ClientBackend,
    pub payer: Arc<Keypair>,
    pub commitment: CommitmentConfig,
    pub cluster: RpcCluster,
    pub default_tx_version: TransactionVersion,
    /// litesvm keeps only compiled inner instructions with no message keys,
    /// so `get_transaction` couldn't resolve them after the fact; the
    /// resolved form is remembered here at `send_transaction` time.
    inner_instruction_cache: Arc<Mutex<HashMap<Signature, Option<Vec<NaclacInnerInstruction>>>>>,
}

impl NaclacProvider {
    pub fn new_litesvm(payer: Keypair) -> Result<Self, NaclacClientError> {
        let mut svm = LiteSVM::new();
        let payer_pubkey = payer.pubkey();
        svm.airdrop(&payer_pubkey, 1_000_000_000_000).map_err(|e| {
            NaclacClientError::LiteSvmError(format!("Initial payer airdrop failed: {:?}", e))
        })?;
        auto_load_workspace_programs(&mut svm)?;
        Ok(Self {
            backend: ClientBackend::LiteSVM(Arc::new(Mutex::new(svm))),
            payer: Arc::new(payer),
            commitment: CommitmentConfig::confirmed(),
            cluster: RpcCluster::Litesvm,
            default_tx_version: TransactionVersion::V1,
            inner_instruction_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn new_rpc(url: &str, payer: Keypair) -> Result<Self, NaclacClientError> {
        Ok(Self {
            backend: ClientBackend::Rpc(Arc::new(RpcClient::new(url.to_string()))),
            payer: Arc::new(payer),
            commitment: CommitmentConfig::confirmed(),
            cluster: cluster_from_url(url),
            default_tx_version: TransactionVersion::V1,
            inner_instruction_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn with_tx_version(mut self, version: TransactionVersion) -> Self {
        self.default_tx_version = version;
        self
    }

    pub fn new(cluster: &str, payer: Keypair) -> Result<Self, NaclacClientError> {
        match cluster {
            "litesvm" => Self::new_litesvm(payer),
            "localnet" => Self::new_rpc("http://127.0.0.1:8899", payer),
            "devnet" => Self::new_rpc("https://api.devnet.solana.com", payer),
            "testnet" => Self::new_rpc("https://api.testnet.solana.com", payer),
            "mainnet" | "mainnet-beta" => {
                Self::new_rpc("https://api.mainnet-beta.solana.com", payer)
            }
            url => Self::new_rpc(url, payer),
        }
    }

    pub fn send_transaction(
        &self,
        vtx: &VersionedTransaction,
        account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let result = match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                match svm.send_transaction(vtx.clone()) {
                    Ok(meta) => {
                        let inner_instructions = resolve_litesvm_inner_instructions(
                            vtx.message.static_account_keys(),
                            &meta.inner_instructions,
                        );
                        if let Ok(mut cache) = self.inner_instruction_cache.lock() {
                            cache.insert(meta.signature, inner_instructions.clone());
                        }
                        Ok(NaclacTransactionMetadata {
                            signature: meta.signature,
                            logs: meta.logs,
                            compute_units_consumed: meta.compute_units_consumed,
                            return_data: meta.return_data.data,
                            inner_instructions,
                        })
                    }
                    Err(failed_meta) => Err(translate_transaction_failure(
                        &failed_meta.err,
                        failed_meta.meta.logs,
                        account_names,
                    )),
                }
            }
            ClientBackend::Rpc(client) => {
                match client
                    .send_and_confirm_transaction_with_spinner_and_commitment(vtx, self.commitment)
                {
                    Ok(sig) => {
                        let mut logs = Vec::new();
                        let mut compute_units_consumed = 0;
                        let mut return_data = Vec::new();
                        let mut inner_instructions = None;
                        if let Ok(tx_response) = client.get_transaction_with_config(
                            &sig,
                            solana_rpc_client_api::config::RpcTransactionConfig {
                                encoding: Some(
                                    solana_transaction_status::UiTransactionEncoding::Base64,
                                ),
                                commitment: Some(self.commitment),
                                max_supported_transaction_version: Some(1),
                            },
                        ) {
                            inner_instructions =
                                inner_instructions_from_rpc(&tx_response.transaction);
                            if let Some(meta) = tx_response.transaction.meta {
                                if let solana_transaction_status::option_serializer::OptionSerializer::Some(l) = meta.log_messages {
                                    logs = l;
                                }
                                compute_units_consumed = meta.compute_units_consumed.unwrap_or(0);
                                if let solana_transaction_status::option_serializer::OptionSerializer::Some(rd) = meta.return_data {
                                    use base64::Engine;
                                    return_data = base64::engine::general_purpose::STANDARD
                                        .decode(&rd.data.0)
                                        .unwrap_or_default();
                                }
                            }
                        }
                        Ok(NaclacTransactionMetadata {
                            signature: sig,
                            logs,
                            compute_units_consumed,
                            return_data,
                            inner_instructions,
                        })
                    }
                    Err(client_err) => {
                        let mut translated = format!("{:?}", client_err);
                        let mut ins_err = InstructionError::GenericError;
                        let mut tx_err_opt = None;
                        let mut tx_logs = Vec::new();

                        if let solana_rpc_client_api::client_error::ErrorKind::TransactionError(
                            tx_err,
                        ) = &*client_err.kind
                        {
                            tx_err_opt = Some(tx_err.clone());
                        } else if let solana_rpc_client_api::client_error::ErrorKind::RpcError(
                            rpc_err,
                        ) = &*client_err.kind
                        {
                            let err_str = format!("{:?}", rpc_err);
                            if let Some(custom_idx) = err_str.find("Custom(") {
                                let start = custom_idx + "Custom(".len();
                                if let Some(end) = err_str[start..].find(')') {
                                    if let Ok(code) = err_str[start..start + end].parse::<u32>() {
                                        tx_err_opt = Some(TransactionError::InstructionError(
                                            0,
                                            InstructionError::Custom(code),
                                        ));
                                    }
                                }
                            }
                            if let Some(logs_idx) = err_str.find("logs: Some([") {
                                let start = logs_idx + "logs: Some([".len();
                                if let Some(end) = err_str[start..].find("])") {
                                    let logs_block = &err_str[start..start + end];
                                    for log_line in logs_block.split("\", \"") {
                                        let clean_line =
                                            log_line.trim_matches('"').replace("\\\"", "\"");
                                        if !clean_line.is_empty() {
                                            tx_logs.push(clean_line);
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(TransactionError::InstructionError(_ix_index, ix_err)) =
                            tx_err_opt
                        {
                            ins_err = ix_err.clone();
                            if let InstructionError::Custom(code) = ix_err {
                                translated = translate_error_code(code, account_names, &tx_logs);
                            }
                        }

                        Err(NaclacClientError::TransactionFailed {
                            instruction_err: ins_err,
                            logs: tx_logs,
                            translated_msg: translated,
                        })
                    }
                }
            }
        };

        if let Ok(meta) = &result {
            write_profile_log(vtx, meta);
        }

        result
    }

    /// Runs a transaction without committing any state changes — same error
    /// translation as `send_transaction`, just never persisted.
    pub fn simulate_transaction(
        &self,
        vtx: &VersionedTransaction,
        account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                match svm.simulate_transaction(vtx.clone()) {
                    Ok(info) => Ok(NaclacTransactionMetadata {
                        signature: info.meta.signature,
                        logs: info.meta.logs,
                        compute_units_consumed: info.meta.compute_units_consumed,
                        return_data: info.meta.return_data.data,
                        inner_instructions: resolve_litesvm_inner_instructions(
                            vtx.message.static_account_keys(),
                            &info.meta.inner_instructions,
                        ),
                    }),
                    Err(failed_meta) => Err(translate_transaction_failure(
                        &failed_meta.err,
                        failed_meta.meta.logs,
                        account_names,
                    )),
                }
            }
            ClientBackend::Rpc(client) => {
                let response = client
                    .simulate_transaction_with_config(
                        vtx,
                        solana_rpc_client_api::config::RpcSimulateTransactionConfig {
                            inner_instructions: true,
                            ..Default::default()
                        },
                    )
                    .map_err(|e| {
                        NaclacClientError::RpcError(format!(
                            "Failed to simulate transaction: {}",
                            e
                        ))
                    })?;
                let sim = response.value;
                let logs = sim.logs.unwrap_or_default();
                if let Some(err) = sim.err {
                    let err: TransactionError = err.into();
                    return Err(translate_transaction_failure(&err, logs, account_names));
                }
                let compute_units_consumed = sim.units_consumed.unwrap_or(0);
                let return_data = sim
                    .return_data
                    .map(|rd| {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD
                            .decode(&rd.data.0)
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let inner_instructions = sim.inner_instructions.as_ref().and_then(|list| {
                    let keys: Vec<solana_address::Address> = vtx
                        .message
                        .static_account_keys()
                        .iter()
                        .map(|key| solana_address::Address::new_from_array(key.to_bytes()))
                        .collect();
                    resolve_ui_inner_instructions(&keys, list)
                });
                Ok(NaclacTransactionMetadata {
                    signature: Signature::default(),
                    logs,
                    compute_units_consumed,
                    return_data,
                    inner_instructions,
                })
            }
        }
    }

    /// Looks up an already-processed transaction by signature — used to
    /// fetch a confirmed transaction's logs after the fact (the same
    /// race-free pattern the generated TS clients' `.rpc()` already uses:
    /// read logs from a transaction that has already landed, rather than
    /// racing a live subscription against it).
    pub fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                match svm.get_transaction(signature) {
                    Some(Ok(meta)) => Ok(NaclacTransactionMetadata {
                        signature: meta.signature,
                        logs: meta.logs.clone(),
                        compute_units_consumed: meta.compute_units_consumed,
                        return_data: meta.return_data.data.clone(),
                        inner_instructions: self
                            .inner_instruction_cache
                            .lock()
                            .ok()
                            .and_then(|cache| cache.get(signature).cloned())
                            .flatten(),
                    }),
                    Some(Err(failed_meta)) => Err(translate_transaction_failure(
                        &failed_meta.err,
                        failed_meta.meta.logs.clone(),
                        None,
                    )),
                    None => Err(NaclacClientError::General(format!(
                        "Transaction {} not found",
                        signature
                    ))),
                }
            }
            ClientBackend::Rpc(client) => {
                let tx_response = client
                    .get_transaction_with_config(
                        signature,
                        solana_rpc_client_api::config::RpcTransactionConfig {
                            encoding: Some(
                                solana_transaction_status::UiTransactionEncoding::Base64,
                            ),
                            commitment: Some(self.commitment),
                            max_supported_transaction_version: Some(1),
                        },
                    )
                    .map_err(|e| {
                        NaclacClientError::RpcError(format!(
                            "Failed to fetch transaction {}: {}",
                            signature, e
                        ))
                    })?;
                let mut logs = Vec::new();
                let mut compute_units_consumed = 0;
                let mut return_data = Vec::new();
                let inner_instructions = inner_instructions_from_rpc(&tx_response.transaction);
                if let Some(meta) = tx_response.transaction.meta {
                    if let solana_transaction_status::option_serializer::OptionSerializer::Some(l) =
                        meta.log_messages
                    {
                        logs = l;
                    }
                    compute_units_consumed = meta.compute_units_consumed.unwrap_or(0);
                    if let solana_transaction_status::option_serializer::OptionSerializer::Some(
                        rd,
                    ) = meta.return_data
                    {
                        use base64::Engine;
                        return_data = base64::engine::general_purpose::STANDARD
                            .decode(&rd.data.0)
                            .unwrap_or_default();
                    }
                }
                Ok(NaclacTransactionMetadata {
                    signature: *signature,
                    logs,
                    compute_units_consumed,
                    return_data,
                    inner_instructions,
                })
            }
        }
    }

    pub fn get_account(
        &self,
        address: &solana_address::Address,
    ) -> Result<Account, NaclacClientError> {
        let pubkey = Address::new_from_array(address.to_bytes());

        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                svm.get_account(&pubkey)
                    .ok_or_else(|| NaclacClientError::AccountNotFound(address.to_string()))
            }
            ClientBackend::Rpc(client) => client
                .get_account_with_commitment(&pubkey, self.commitment)
                .map_err(|e| {
                    NaclacClientError::RpcError(format!("Failed to get account {}: {}", address, e))
                })?
                .value
                .ok_or_else(|| NaclacClientError::AccountNotFound(address.to_string())),
        }
    }

    /// Returns the raw data bytes for an on-chain account. Returns an error if the account does not exist.
    pub fn get_account_data(
        &self,
        address: &solana_address::Address,
    ) -> Result<Vec<u8>, NaclacClientError> {
        self.get_account(address).map(|acc| acc.data)
    }

    /// Fetches all accounts owned by `program_id`, optionally filtered by a discriminator prefix.
    /// Returns `(address, raw_data)` pairs. Useful for indexing all accounts of a given type.
    pub fn get_program_accounts(
        &self,
        program_id: &solana_address::Address,
        discriminator: Option<&[u8]>,
    ) -> Result<Vec<(solana_address::Address, Vec<u8>)>, NaclacClientError> {
        let pubkey = Address::new_from_array(program_id.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                let mut results = Vec::new();
                for (addr, account) in svm.get_program_accounts(&pubkey) {
                    if let Some(disc) = discriminator {
                        if account.data.len() < disc.len() || &account.data[..disc.len()] != disc {
                            continue;
                        }
                    }
                    results.push((
                        solana_address::Address::new_from_array(addr.to_bytes()),
                        account.data,
                    ));
                }
                Ok(results)
            }
            ClientBackend::Rpc(client) => {
                use {
                    solana_account_decoder::{UiAccountData, UiAccountEncoding},
                    solana_rpc_client_api::{
                        config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
                        filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
                    },
                };

                let filters = discriminator.map(|disc| {
                    vec![RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Bytes(disc.to_vec()),
                    ))]
                });
                let config = RpcProgramAccountsConfig {
                    filters,
                    account_config: RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        commitment: Some(self.commitment),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let ui_accounts = client
                    .get_program_ui_accounts_with_config(&pubkey, config)
                    .map_err(|e| {
                        NaclacClientError::RpcError(format!(
                            "get_program_accounts failed for {}: {}",
                            program_id, e
                        ))
                    })?;
                let mut results = Vec::new();
                for (pk, ui_acct) in ui_accounts {
                    let bytes = match &ui_acct.data {
                        UiAccountData::Binary(encoded, _) => {
                            use base64::Engine;
                            base64::engine::general_purpose::STANDARD
                                .decode(encoded)
                                .unwrap_or_default()
                        }
                        UiAccountData::Json(_) | UiAccountData::LegacyBinary(_) => continue,
                    };
                    let addr = solana_address::Address::new_from_array(pk.to_bytes());
                    results.push((addr, bytes));
                }
                Ok(results)
            }
        }
    }

    pub fn get_balance(&self, address: &solana_address::Address) -> Result<u64, NaclacClientError> {
        let pubkey = Address::new_from_array(address.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                Ok(svm.get_balance(&pubkey).unwrap_or(0))
            }
            ClientBackend::Rpc(client) => client
                .get_balance_with_commitment(&pubkey, self.commitment)
                .map_err(|e| {
                    NaclacClientError::RpcError(format!(
                        "Failed to get balance of {}: {}",
                        address, e
                    ))
                })
                .map(|res| res.value),
        }
    }

    /// Returns `(amount, decimals)` for an SPL Token or Token-2022 account
    /// (branches on the account's owner — both programs' base account
    /// layout, `{ mint, amount, ... }` at a fixed offset, is unpackable via
    /// either crate's own `Account::unpack`/`Mint::unpack`; Token-2022's TLV
    /// extensions live after that fixed-size prefix and don't affect it). A
    /// token account only stores `amount` and which `mint` it belongs to —
    /// `decimals` lives on the mint itself, so this fetches both accounts.
    pub fn get_token_balance(
        &self,
        address: &solana_address::Address,
    ) -> Result<(u64, u8), NaclacClientError> {
        use solana_program_pack::Pack;

        let account = self.get_account(address)?;
        let owner_bytes = account.owner.to_bytes();

        let (amount, mint_bytes) = if owner_bytes == spl_token::id().to_bytes() {
            let token_account = spl_token::state::Account::unpack(&account.data).map_err(|e| {
                NaclacClientError::General(format!(
                    "Failed to unpack token account {}: {}",
                    address, e
                ))
            })?;
            (token_account.amount, token_account.mint.to_bytes())
        } else if owner_bytes == spl_token_2022::id().to_bytes() {
            let token_account =
                spl_token_2022::state::Account::unpack(&account.data).map_err(|e| {
                    NaclacClientError::General(format!(
                        "Failed to unpack token-2022 account {}: {}",
                        address, e
                    ))
                })?;
            (token_account.amount, token_account.mint.to_bytes())
        } else {
            return Err(NaclacClientError::General(format!(
                "{} is not a Token or Token-2022 account (owner {})",
                address, account.owner
            )));
        };

        let mint_address = solana_address::Address::new_from_array(mint_bytes);
        let mint_data = self.get_account_data(&mint_address)?;
        let decimals = if owner_bytes == spl_token::id().to_bytes() {
            spl_token::state::Mint::unpack(&mint_data)
                .map_err(|e| {
                    NaclacClientError::General(format!(
                        "Failed to unpack mint {}: {}",
                        mint_address, e
                    ))
                })?
                .decimals
        } else {
            spl_token_2022::state::Mint::unpack(&mint_data)
                .map_err(|e| {
                    NaclacClientError::General(format!(
                        "Failed to unpack mint {}: {}",
                        mint_address, e
                    ))
                })?
                .decimals
        };

        Ok((amount, decimals))
    }

    pub fn get_latest_blockhash(&self) -> Result<Hash, NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                Ok(svm.latest_blockhash())
            }
            ClientBackend::Rpc(client) => client
                .get_latest_blockhash_with_commitment(self.commitment)
                .map_err(|e| {
                    NaclacClientError::RpcError(format!("Failed to get latest blockhash: {}", e))
                })
                .map(|res| res.0),
        }
    }

    pub fn get_minimum_balance_for_rent_exemption(
        &self,
        size: usize,
    ) -> Result<u64, NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                Ok(svm.minimum_balance_for_rent_exemption(size))
            }
            ClientBackend::Rpc(client) => client
                .get_minimum_balance_for_rent_exemption(size)
                .map_err(|e| {
                    NaclacClientError::RpcError(format!("Failed to get rent exemption: {}", e))
                }),
        }
    }

    pub fn airdrop(
        &self,
        address: &solana_address::Address,
        lamports: u64,
    ) -> Result<(), NaclacClientError> {
        let pubkey = Address::new_from_array(address.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                svm.airdrop(&pubkey, lamports).map_err(|e| {
                    NaclacClientError::LiteSvmError(format!("Airdrop failed: {:?}", e))
                })?;
                Ok(())
            }
            ClientBackend::Rpc(client) => {
                // Devnet's real faucet is rate-limited hard enough that a
                // single over-large request routinely fails outright; cap
                // requests there to what it reliably grants. Localnet's own
                // faucet has no such limit (it's a local validator), and
                // mainnet has no faucet at all — both pass the amount
                // through unchanged.
                const DEVNET_MAX_AIRDROP_LAMPORTS: u64 = 5_000_000_000;
                let lamports = if self.cluster == RpcCluster::Devnet {
                    lamports.min(DEVNET_MAX_AIRDROP_LAMPORTS)
                } else {
                    lamports
                };
                let sig = client.request_airdrop(&pubkey, lamports).map_err(|e| {
                    NaclacClientError::RpcError(format!("Airdrop request failed: {}", e))
                })?;
                // Poll confirmation
                let mut confirmed = false;
                for _ in 0..15 {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Ok(statuses) = client.get_signature_statuses(&[sig]) {
                        if let Some(Some(status)) = statuses.value.first() {
                            if status.confirmations.is_some()
                                || status.confirmation_status.is_some()
                            {
                                confirmed = true;
                                break;
                            }
                        }
                    }
                }
                if !confirmed {
                    return Err(NaclacClientError::RpcError(
                        "Airdrop confirmation timed out".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Advances the litesvm backend's blockhash. Two transactions with
    /// identical accounts, args, and signers produce identical signatures
    /// under the same blockhash — real Solana rejects the resubmission as
    /// `AlreadyProcessed`, and litesvm faithfully reproduces that, since it
    /// doesn't auto-rotate its blockhash per transaction the way a real
    /// validator rotates per slot. Call this between two otherwise-identical
    /// transactions in a test.
    pub fn expire_blockhash(&self) -> Result<(), NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                svm.expire_blockhash();
                Ok(())
            }
            ClientBackend::Rpc(_) => Err(NaclacClientError::General(
                "expire_blockhash is only supported on the litesvm backend".to_string(),
            )),
        }
    }

    /// Sets an account's lamport balance directly, bypassing normal transfer
    /// execution (and its rent-exemption rule) — `airdrop` goes through a
    /// real System Program transfer, which the runtime rejects if it would
    /// leave a new account underfunded. Only supported on the litesvm
    /// backend, since it writes ledger state that isn't reachable through a
    /// real transaction.
    pub fn set_account_lamports(
        &self,
        address: &solana_address::Address,
        lamports: u64,
    ) -> Result<(), NaclacClientError> {
        let pubkey = Address::new_from_array(address.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                let mut account = svm.get_account(&pubkey).unwrap_or_default();
                account.lamports = lamports;
                svm.set_account(pubkey, account).map_err(|e| {
                    NaclacClientError::LiteSvmError(format!("set_account failed: {:?}", e))
                })?;
                Ok(())
            }
            ClientBackend::Rpc(_) => Err(NaclacClientError::General(
                "set_account_lamports is only supported on the litesvm backend".to_string(),
            )),
        }
    }

    /// Overwrites the litesvm backend's `Clock` sysvar with `unix_timestamp`,
    /// leaving every other `Clock` field (slot, epoch, ...) untouched.
    /// Litesvm's default `Clock` starts at `unix_timestamp = 0` and nothing
    /// advances it on its own, so any test relying on real-looking on-chain
    /// timestamps (e.g. an emitted event's `timestamp` field, or a
    /// rate-limit computed from elapsed time) needs to set this explicitly.
    /// Only supported on the litesvm backend, since a real RPC target's
    /// clock reflects genuine chain time and can't be overwritten this way.
    pub fn set_clock_unix_timestamp(&self, unix_timestamp: i64) -> Result<(), NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                let mut clock: solana_clock::Clock = svm.get_sysvar();
                clock.unix_timestamp = unix_timestamp;
                svm.set_sysvar(&clock);
                Ok(())
            }
            ClientBackend::Rpc(_) => Err(NaclacClientError::General(
                "set_clock_unix_timestamp is only supported on the litesvm backend".to_string(),
            )),
        }
    }

    /// Directly injects an account into ledger state — data, owner, and
    /// lamports all explicitly controlled — bypassing normal account
    /// creation entirely. Only supported on the litesvm backend, since it
    /// writes ledger state that isn't reachable through a real transaction.
    /// Needed for fixtures owned by a program this test doesn't itself
    /// deploy or control (e.g. a PDA whose real owner is a different,
    /// possibly not-yet-built program) — there is no other way to give such
    /// an account real backing data under test.
    pub fn set_account(
        &self,
        address: &solana_address::Address,
        data: Vec<u8>,
        owner: &solana_address::Address,
        lamports: u64,
    ) -> Result<(), NaclacClientError> {
        let pubkey = Address::new_from_array(address.to_bytes());
        let owner_pubkey = Address::new_from_array(owner.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                let account = Account {
                    lamports,
                    data,
                    owner: owner_pubkey,
                    executable: false,
                    rent_epoch: 0,
                };
                svm.set_account(pubkey, account).map_err(|e| {
                    NaclacClientError::LiteSvmError(format!("set_account failed: {:?}", e))
                })?;
                Ok(())
            }
            ClientBackend::Rpc(_) => Err(NaclacClientError::General(
                "set_account is only supported on the litesvm backend".to_string(),
            )),
        }
    }

    pub fn add_program(
        &self,
        program_id: &solana_address::Address,
        path: &str,
    ) -> Result<(), NaclacClientError> {
        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                let pubkey = Address::new_from_array(program_id.to_bytes());
                svm.add_program_from_file(pubkey, path).map_err(|e| {
                    NaclacClientError::LiteSvmError(format!(
                        "Failed to load program binary into LiteSVM: {:?}",
                        e
                    ))
                })?;
                Ok(())
            }
            ClientBackend::Rpc(_) => {
                // Loading program binary is a no-op on a live RPC network
                Ok(())
            }
        }
    }

    /// Fetches a deployed program's executable bytes from a live cluster and
    /// loads them into this litesvm instance at `program_id` — for CPI
    /// targets that live on a real network (e.g. mainnet) rather than in
    /// this workspace. Mirrors `solana program dump`'s exact mechanism
    /// (`agave/cli/src/program.rs`, `process_dump`): non-upgradeable-loader
    /// accounts are used as-is; upgradeable-loader accounts are followed to
    /// their ProgramData account and stripped of its
    /// `UpgradeableLoaderState::size_of_programdata_metadata()`-byte header.
    /// LoaderV4 is not supported, matching `dump`'s own scope. Only
    /// supported on the litesvm backend.
    ///
    /// The fetched bytes are cached process-wide, keyed by
    /// `(program_id, cluster_url)` — a test binary that calls this
    /// repeatedly (e.g. once per `#[test]`, each building its own fresh
    /// litesvm instance) only hits the network once; every later call for
    /// the same program+cluster reuses the cached bytes with no RPC call at
    /// all. The underlying `get_account_with_commitment` calls are retried
    /// up to 5 times (transient RPC/network errors only — a real "account
    /// not found" or "not an SBF program" is not retried, since retrying
    /// can't fix that) before giving up with a clear error.
    pub fn add_program_from_cluster(
        &self,
        program_id: &solana_address::Address,
        cluster_url: &str,
    ) -> Result<(), NaclacClientError> {
        use {
            solana_loader_v3_interface::state::UpgradeableLoaderState,
            solana_sdk_ids::{bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable},
            wincode::DeserializeOwned,
        };

        const MAX_FETCH_ATTEMPTS: u32 = 5;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

        fn fetch_account_with_retry(
            fetch_client: &RpcClient,
            pubkey: &Address,
            commitment: CommitmentConfig,
            what: &str,
        ) -> Result<Option<Account>, NaclacClientError> {
            let mut last_err = None;
            for attempt in 1..=MAX_FETCH_ATTEMPTS {
                match fetch_client.get_account_with_commitment(pubkey, commitment) {
                    Ok(resp) => return Ok(resp.value),
                    Err(e) => {
                        last_err = Some(e.to_string());
                        if attempt < MAX_FETCH_ATTEMPTS {
                            std::thread::sleep(RETRY_DELAY);
                        }
                    }
                }
            }
            Err(NaclacClientError::RpcError(format!(
                "Failed to fetch {} {} after {} attempts: {}",
                what,
                pubkey,
                MAX_FETCH_ATTEMPTS,
                last_err.unwrap_or_default()
            )))
        }

        type ProgramCacheKey = (solana_address::Address, String);
        static PROGRAM_BYTES_CACHE: OnceLock<Mutex<HashMap<ProgramCacheKey, Vec<u8>>>> =
            OnceLock::new();
        let cache = PROGRAM_BYTES_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let cache_key: ProgramCacheKey = (*program_id, cluster_url.to_string());

        match &self.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let pubkey = Address::new_from_array(program_id.to_bytes());

                let cached = cache
                    .lock()
                    .map_err(|e| {
                        NaclacClientError::General(format!(
                            "Failed to lock program-bytes cache: {}",
                            e
                        ))
                    })?
                    .get(&cache_key)
                    .cloned();

                let elf_bytes = if let Some(cached_bytes) = cached {
                    cached_bytes
                } else {
                    let fetch_client = RpcClient::new(cluster_url.to_string());

                    let account = fetch_account_with_retry(
                        &fetch_client,
                        &pubkey,
                        self.commitment,
                        "program account",
                    )?
                    .ok_or_else(|| NaclacClientError::AccountNotFound(program_id.to_string()))?;

                    let elf_bytes = if account.owner == bpf_loader::id()
                        || account.owner == bpf_loader_deprecated::id()
                    {
                        account.data
                    } else if account.owner == bpf_loader_upgradeable::id() {
                        let Ok(UpgradeableLoaderState::Program {
                            programdata_address,
                        }) = UpgradeableLoaderState::deserialize_from(&account.data[..])
                        else {
                            return Err(NaclacClientError::General(format!(
                                "{} is not an upgradeable-loader program account",
                                program_id
                            )));
                        };

                        let programdata_account = fetch_account_with_retry(
                            &fetch_client,
                            &programdata_address,
                            self.commitment,
                            "ProgramData account",
                        )?
                        .ok_or_else(|| {
                            NaclacClientError::General(format!(
                                "Program {} has been closed (ProgramData account missing)",
                                program_id
                            ))
                        })?;

                        if !matches!(
                            UpgradeableLoaderState::deserialize_from(&programdata_account.data[..]),
                            Ok(UpgradeableLoaderState::ProgramData { .. })
                        ) {
                            return Err(NaclacClientError::General(format!(
                                "Program {} has been closed",
                                program_id
                            )));
                        }

                        let offset = UpgradeableLoaderState::size_of_programdata_metadata();
                        programdata_account.data[offset..].to_vec()
                    } else {
                        return Err(NaclacClientError::General(format!(
                            "{} is not an SBF program (owner {} is not a supported loader — \
                             LoaderV4 is not supported, matching `solana program dump`)",
                            program_id, account.owner
                        )));
                    };

                    cache
                        .lock()
                        .map_err(|e| {
                            NaclacClientError::General(format!(
                                "Failed to lock program-bytes cache: {}",
                                e
                            ))
                        })?
                        .insert(cache_key, elf_bytes.clone());

                    elf_bytes
                };

                let mut svm = svm_lock.lock().map_err(|e| {
                    NaclacClientError::General(format!("Failed to lock LiteSVM: {}", e))
                })?;
                svm.add_program(pubkey, &elf_bytes).map_err(|e| {
                    NaclacClientError::LiteSvmError(format!(
                        "Failed to load fetched program bytes into LiteSVM: {:?}",
                        e
                    ))
                })?;
                Ok(())
            }
            ClientBackend::Rpc(_) => Err(NaclacClientError::General(
                "add_program_from_cluster is only supported on the litesvm backend".to_string(),
            )),
        }
    }
}

static PROFILE_WRITE_LOCK: Mutex<()> = Mutex::new(());

fn write_profile_log(tx: &VersionedTransaction, meta: &NaclacTransactionMetadata) {
    if let Ok(file_path) = std::env::var("NACLAC_PROFILE_FILE") {
        let mut instructions = Vec::new();
        let message = &tx.message;
        let static_account_keys = message.static_account_keys();
        for ix in message.instructions() {
            let program_id_index = ix.program_id_index as usize;
            if program_id_index < static_account_keys.len() {
                let prog_id = static_account_keys[program_id_index];
                let data_hex = hex::encode(&ix.data);
                instructions.push(serde_json::json!({
                    "program_id": prog_id.to_string(),
                    "data": data_hex,
                }));
            }
        }

        let log_entry = serde_json::json!({
            "instructions": instructions,
            "compute_units_consumed": meta.compute_units_consumed,
            "logs": meta.logs,
        });

        if let Ok(log_str) = serde_json::to_string(&log_entry) {
            use std::{fs::OpenOptions, io::Write};
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _guard = PROFILE_WRITE_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
            {
                let _ = writeln!(file, "{}", log_str);
            }
        }
    }
}
