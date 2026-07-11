use crate::error::{translate_error_code, NaclacClientError};
use litesvm::LiteSVM;
use sha2::Digest;
use solana_account::Account;
use solana_commitment_config::CommitmentConfig;
use solana_keypair::Keypair;
use solana_program::{hash::Hash, instruction::InstructionError, pubkey::Pubkey};
use solana_rpc_client::rpc_client::RpcClient;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction::TransactionError;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct NaclacTransactionMetadata {
    pub signature: Signature,
    pub logs: Vec<String>,
    pub compute_units_consumed: u64,
}

fn extract_event_bytes(parts: &[&str]) -> Option<(Vec<u8>, Vec<u8>)> {
    use base64::Engine;
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(parts[0])
            .ok()?;
        if bytes.len() >= 8 {
            let disc = bytes[0..8].to_vec();
            let payload = bytes[8..].to_vec();
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
    /// Parses Borsh-serialized events of type `T` from the transaction logs.
    pub fn parse_events_borsh<T: borsh::BorshDeserialize>(
        &self,
    ) -> Result<Vec<T>, NaclacClientError> {
        let full_name = std::any::type_name::<T>();
        let struct_name = full_name.split("::").last().unwrap();
        let preimage = format!("event:{}", struct_name);

        let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
        hasher.update(preimage.as_bytes());
        let hash_result = hasher.finalize();
        let mut expected_disc = [0u8; 8];
        expected_disc.copy_from_slice(&hash_result[0..8]);

        let mut events = Vec::new();
        for log in &self.logs {
            if let Some(data_str) = log.strip_prefix("Program data: ") {
                let parts: Vec<&str> = data_str.split_whitespace().collect();
                if let Some((disc_bytes, payload_bytes)) = extract_event_bytes(&parts) {
                    if disc_bytes == expected_disc {
                        let mut reader = &payload_bytes[..];
                        if let Ok(event) = T::deserialize(&mut reader) {
                            events.push(event);
                        }
                    }
                }
            }
        }
        Ok(events)
    }

    /// Parses Zero-Copy (bytemuck Pod) events of type `T` from the transaction logs.
    pub fn parse_events_zero_copy<T: bytemuck::Pod>(&self) -> Result<Vec<T>, NaclacClientError> {
        let full_name = std::any::type_name::<T>();
        let struct_name = full_name.split("::").last().unwrap();
        let preimage = format!("event:{}", struct_name);

        let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
        hasher.update(preimage.as_bytes());
        let hash_result = hasher.finalize();
        let mut expected_disc = [0u8; 8];
        expected_disc.copy_from_slice(&hash_result[0..8]);

        let mut events = Vec::new();
        for log in &self.logs {
            if let Some(data_str) = log.strip_prefix("Program data: ") {
                let parts: Vec<&str> = data_str.split_whitespace().collect();
                if let Some((disc_bytes, payload_bytes)) = extract_event_bytes(&parts) {
                    if disc_bytes == expected_disc
                        && payload_bytes.len() >= std::mem::size_of::<T>()
                    {
                        if let Ok(casted) =
                            bytemuck::try_from_bytes(&payload_bytes[0..std::mem::size_of::<T>()])
                        {
                            events.push(*casted);
                        }
                    }
                }
            }
        }
        Ok(events)
    }
}

#[derive(Clone)]
pub enum ClientBackend {
    LiteSVM(Arc<Mutex<LiteSVM>>),
    Rpc(Arc<RpcClient>),
}

#[derive(Clone)]
pub struct NaclacProvider {
    pub backend: ClientBackend,
    pub payer: Arc<Keypair>,
    pub commitment: CommitmentConfig,
}

impl NaclacProvider {
    pub fn new_litesvm(payer: Keypair) -> Self {
        let mut svm = LiteSVM::new();
        let payer_pubkey = payer.pubkey();
        svm.airdrop(&payer_pubkey, 1_000_000_000_000).unwrap();
        Self {
            backend: ClientBackend::LiteSVM(Arc::new(Mutex::new(svm))),
            payer: Arc::new(payer),
            commitment: CommitmentConfig::confirmed(),
        }
    }

    pub fn new_rpc(url: &str, payer: Keypair) -> Self {
        Self {
            backend: ClientBackend::Rpc(Arc::new(RpcClient::new(url.to_string()))),
            payer: Arc::new(payer),
            commitment: CommitmentConfig::confirmed(),
        }
    }

    pub fn new(cluster: &str, payer: Keypair) -> Self {
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
                    Ok(meta) => Ok(NaclacTransactionMetadata {
                        signature: meta.signature,
                        logs: meta.logs,
                        compute_units_consumed: meta.compute_units_consumed,
                    }),
                    Err(failed_meta) => {
                        let (ins_err, err_code) = match &failed_meta.err {
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
                        let translated = if let Some(code) = err_code {
                            translate_error_code(code, account_names)
                        } else {
                            format!("{:?}", failed_meta.err)
                        };
                        Err(NaclacClientError::TransactionFailed {
                            instruction_err: ins_err,
                            logs: failed_meta.meta.logs,
                            translated_msg: translated,
                        })
                    }
                }
            }
            ClientBackend::Rpc(client) => {
                match client
                    .send_and_confirm_transaction_with_spinner_and_commitment(vtx, self.commitment)
                {
                    Ok(sig) => {
                        let mut logs = Vec::new();
                        let mut compute_units_consumed = 0;
                        if let Ok(tx_response) = client.get_transaction_with_config(
                            &sig,
                            solana_rpc_client_api::config::RpcTransactionConfig {
                                encoding: Some(
                                    solana_transaction_status::UiTransactionEncoding::Base64,
                                ),
                                commitment: Some(self.commitment),
                                max_supported_transaction_version: Some(0),
                            },
                        ) {
                            if let Some(meta) = tx_response.transaction.meta {
                                if let solana_transaction_status::option_serializer::OptionSerializer::Some(l) = meta.log_messages {
                                    logs = l;
                                }
                                compute_units_consumed = meta.compute_units_consumed.unwrap_or(0);
                            }
                        }
                        Ok(NaclacTransactionMetadata {
                            signature: sig,
                            logs,
                            compute_units_consumed,
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
                                translated = translate_error_code(code, account_names);
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

    pub fn get_account(
        &self,
        address: &solana_address::Address,
    ) -> Result<Account, NaclacClientError> {
        let pubkey = Pubkey::new_from_array(address.to_bytes());

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

    /// Fetches all accounts owned by `program_id`, optionally filtered by an 8-byte discriminator prefix.
    /// Returns `(address, raw_data)` pairs. Useful for indexing all accounts of a given type.
    pub fn get_program_accounts(
        &self,
        program_id: &solana_address::Address,
        discriminator: Option<&[u8; 8]>,
    ) -> Result<Vec<(solana_address::Address, Vec<u8>)>, NaclacClientError> {
        let pubkey = Pubkey::new_from_array(program_id.to_bytes());
        match &self.backend {
            ClientBackend::LiteSVM(_) => {
                // LiteSVM has no account-iteration API.
                // In test environments, use fetch_xxx() with known addresses instead.
                Ok(Vec::new())
            }
            ClientBackend::Rpc(client) => {
                use solana_account_decoder::{UiAccountData, UiAccountEncoding};
                use solana_rpc_client_api::config::{
                    RpcAccountInfoConfig, RpcProgramAccountsConfig,
                };
                use solana_rpc_client_api::filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};

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
        let pubkey = Pubkey::new_from_array(address.to_bytes());
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
        let pubkey = Pubkey::new_from_array(address.to_bytes());
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
                let pubkey = Pubkey::new_from_array(program_id.to_bytes());
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
}

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
            use std::fs::OpenOptions;
            use std::io::Write;
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
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
