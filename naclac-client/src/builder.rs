use {
    crate::{
        error::NaclacClientError,
        provider::{NaclacProvider, NaclacTransactionMetadata, TransactionVersion},
        AccountMeta,
    },
    solana_address::Address,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_message::{v0, v1, Message as LegacyMessage, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub struct InstructionBuilder<'a> {
    pub provider: &'a NaclacProvider,
    pub program_id: Address,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
    pub account_names: Vec<&'static str>,
    pub extra_signers: Vec<&'a Keypair>,
    pub pre_instructions: Vec<Instruction>,
    pub post_instructions: Vec<Instruction>,
    pub verbose: bool,
    pub version: TransactionVersion,
    pub compute_unit_limit: Option<u32>,
    pub loaded_accounts_data_size_limit: Option<u32>,
    pub priority_fee: Option<u64>,
}

impl<'a> InstructionBuilder<'a> {
    pub fn new(provider: &'a NaclacProvider, program_id: Address, data: Vec<u8>) -> Self {
        Self {
            provider,
            program_id,
            accounts: Vec::new(),
            data,
            account_names: Vec::new(),
            extra_signers: Vec::new(),
            pre_instructions: Vec::new(),
            post_instructions: Vec::new(),
            verbose: false,
            version: provider.default_tx_version,
            compute_unit_limit: None,
            loaded_accounts_data_size_limit: None,
            priority_fee: None,
        }
    }

    pub fn account(mut self, meta: AccountMeta, name: &'static str) -> Self {
        self.accounts.push(meta);
        self.account_names.push(name);
        self
    }

    pub fn accounts(mut self, mut metas: Vec<(AccountMeta, &'static str)>) -> Self {
        for (meta, name) in metas.drain(..) {
            self.accounts.push(meta);
            self.account_names.push(name);
        }
        self
    }

    /// Appends extra accounts that are not statically declared in the IDL.
    /// Used for token-2022 extensions, multi-sig authorities, governance programs, etc.
    pub fn remaining_accounts(mut self, accounts: Vec<AccountMeta>) -> Self {
        for meta in accounts {
            self.accounts.push(meta);
            self.account_names.push("remaining");
        }
        self
    }

    pub fn signer(mut self, keypair: &'a Keypair) -> Self {
        self.extra_signers.push(keypair);
        self
    }

    pub fn v1(mut self) -> Self {
        self.version = TransactionVersion::V1;
        self
    }

    pub fn v0(mut self) -> Self {
        self.version = TransactionVersion::V0;
        self
    }

    pub fn legacy(mut self) -> Self {
        self.version = TransactionVersion::Legacy;
        self
    }

    pub fn version(mut self, version: TransactionVersion) -> Self {
        self.version = version;
        self
    }

    pub fn compute_unit_limit(mut self, limit: u32) -> Self {
        self.compute_unit_limit = Some(limit);
        self
    }

    pub fn loaded_accounts_data_size_limit(mut self, limit: u32) -> Self {
        self.loaded_accounts_data_size_limit = Some(limit);
        self
    }

    pub fn priority_fee(mut self, lamports: u64) -> Self {
        self.priority_fee = Some(lamports);
        self
    }

    /// Prints this instruction's program logs (and compute units consumed) to
    /// stdout once `send_and_confirm` resolves, on both success and failure.
    /// Opt-in and per-call, since `InstructionBuilder` is also used outside
    /// tests, where unconditional log output would just be noise with no way
    /// to quiet it down.
    pub fn log(mut self) -> Self {
        self.verbose = true;
        self
    }

    pub fn pre_instruction(mut self, ix: Instruction) -> Self {
        self.pre_instructions.push(ix);
        self
    }

    pub fn post_instruction(mut self, ix: Instruction) -> Self {
        self.post_instructions.push(ix);
        self
    }

    /// Builds a raw `Instruction` from the current builder state.
    pub fn build_instruction(&self) -> Instruction {
        Instruction {
            program_id: Address::new_from_array(self.program_id.to_bytes()),
            accounts: self
                .accounts
                .iter()
                .map(|meta| solana_instruction::AccountMeta {
                    pubkey: Address::new_from_array(meta.address.to_bytes()),
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data: self.data.clone(),
        }
    }

    /// Builds a signed `VersionedTransaction` using the configured transaction version.
    pub fn transaction(&self) -> Result<VersionedTransaction, NaclacClientError> {
        let ixs = self.all_instructions();
        let payer_pubkey = self.provider.payer.pubkey();
        let recent_blockhash = self.provider.get_latest_blockhash()?;

        let versioned_message = match self.version {
            TransactionVersion::V1 => {
                let mut config = v1::TransactionConfig::empty()
                    .with_compute_unit_limit(self.compute_unit_limit.unwrap_or(1_400_000))
                    .with_loaded_accounts_data_size_limit(
                        self.loaded_accounts_data_size_limit
                            .unwrap_or(64 * 1024 * 1024),
                    );
                if let Some(priority_fee) = self.priority_fee {
                    config = config.with_priority_fee(priority_fee);
                }
                let v1_msg = v1::Message::try_compile_with_config(
                    &payer_pubkey,
                    &ixs,
                    recent_blockhash,
                    config,
                )
                .map_err(|e| {
                    NaclacClientError::General(format!("Failed to compile v1 message: {e:?}"))
                })?;
                VersionedMessage::V1(v1_msg)
            }
            TransactionVersion::V0 => {
                let v0_msg = v0::Message::try_compile(&payer_pubkey, &ixs, &[], recent_blockhash)
                    .map_err(|e| {
                    NaclacClientError::General(format!("Failed to compile v0 message: {e:?}"))
                })?;
                VersionedMessage::V0(v0_msg)
            }
            TransactionVersion::Legacy => {
                let legacy_msg =
                    LegacyMessage::new_with_blockhash(&ixs, Some(&payer_pubkey), &recent_blockhash);
                VersionedMessage::Legacy(legacy_msg)
            }
        };

        let mut signers: Vec<&dyn solana_signer::Signer> = vec![&*self.provider.payer];
        for s in &self.extra_signers {
            signers.push(*s);
        }

        VersionedTransaction::try_new(versioned_message, &signers)
            .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {e:?}")))
    }

    /// Sends a versioned transaction and waits for confirmation.
    pub fn send_and_confirm(&self) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let tx = self.transaction()?;
        let result = self
            .provider
            .send_transaction(&tx, Some(&self.account_names));
        if self.verbose {
            use std::fmt::Write as _;
            let mut block = String::new();
            match &result {
                Ok(meta) => {
                    for line in &meta.logs {
                        let _ = writeln!(block, "{line}");
                    }
                    let _ = writeln!(
                        block,
                        "   💰 Final CU consumed: {}",
                        meta.compute_units_consumed
                    );
                }
                Err(NaclacClientError::TransactionFailed { logs, .. }) => {
                    for line in logs {
                        let _ = writeln!(block, "{line}");
                    }
                }
                Err(_) => {}
            }
            if !block.is_empty() {
                print!("{block}");
            }
        }
        result
    }

    /// Simulates the transaction without committing state changes.
    pub fn simulate(&self) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        let tx = self.transaction()?;
        let result = self
            .provider
            .simulate_transaction(&tx, Some(&self.account_names));
        if self.verbose {
            use std::fmt::Write as _;
            let mut block = String::new();
            match &result {
                Ok(meta) => {
                    for line in &meta.logs {
                        let _ = writeln!(block, "{line}");
                    }
                    let _ = writeln!(
                        block,
                        "   💰 Final CU consumed: {}",
                        meta.compute_units_consumed
                    );
                }
                Err(NaclacClientError::TransactionFailed { logs, .. }) => {
                    for line in logs {
                        let _ = writeln!(block, "{line}");
                    }
                }
                Err(_) => {}
            }
            if !block.is_empty() {
                print!("{block}");
            }
        }
        result
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn all_instructions(&self) -> Vec<Instruction> {
        let mut ixs = Vec::new();
        ixs.extend(self.pre_instructions.clone());
        ixs.push(self.build_instruction());
        ixs.extend(self.post_instructions.clone());
        ixs
    }
}
