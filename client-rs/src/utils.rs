use {
    crate::{
        error::NaclacClientError,
        provider::{NaclacProvider, TransactionVersion},
        token::{Mint, TokenAccount},
    },
    solana_address::{address, Address},
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_message::{v0, v1, Message as LegacyMessage, VersionedMessage},
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub const TOKEN_PROGRAM_ID: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address =
    address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const TOKEN_2022_PROGRAM_ID: Address = address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);
pub const RENT_SYSVAR_ID: Address = Address::new_from_array(solana_sysvar::rent::ID.to_bytes());

pub trait SignerAddressExt {
    fn address(&self) -> Address;
}

impl<T: Signer + ?Sized> SignerAddressExt for T {
    fn address(&self) -> Address {
        Address::new_from_array(self.pubkey().to_bytes())
    }
}

fn to_sdk_instruction(ix: solana_instruction::Instruction) -> Instruction {
    Instruction {
        program_id: Address::new_from_array(ix.program_id.to_bytes()),
        accounts: ix
            .accounts
            .into_iter()
            .map(|meta| solana_instruction::AccountMeta {
                pubkey: Address::new_from_array(meta.pubkey.to_bytes()),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: ix.data,
    }
}

fn compile_provider_message(
    provider: &NaclacProvider,
    ixs: &[Instruction],
) -> Result<VersionedMessage, NaclacClientError> {
    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    match provider.default_tx_version {
        TransactionVersion::V1 => {
            let config = v1::TransactionConfig::empty()
                .with_compute_unit_limit(1_400_000)
                .with_loaded_accounts_data_size_limit(64 * 1024 * 1024);
            let v1_msg =
                v1::Message::try_compile_with_config(&payer_pubkey, ixs, recent_blockhash, config)
                    .map_err(|e| {
                        NaclacClientError::General(format!("Failed to compile v1 message: {:?}", e))
                    })?;
            Ok(VersionedMessage::V1(v1_msg))
        }
        TransactionVersion::V0 => {
            let v0_msg = v0::Message::try_compile(&payer_pubkey, ixs, &[], recent_blockhash)
                .map_err(|e| {
                    NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e))
                })?;
            Ok(VersionedMessage::V0(v0_msg))
        }
        TransactionVersion::Legacy => {
            let legacy_msg =
                LegacyMessage::new_with_blockhash(ixs, Some(&payer_pubkey), &recent_blockhash);
            Ok(VersionedMessage::Legacy(legacy_msg))
        }
    }
}

/// Directory holding each program's `<name>.so` next to its
/// `<name>-keypair.json` for the naclac workspace rooted at `workspace_root`
/// (the directory containing `Naclac.toml`). Must stay identical to
/// `naclac_client_gen::deploy_dir`, which `naclac build` writes to.
pub fn workspace_deploy_dir(workspace_root: &std::path::Path) -> std::path::PathBuf {
    workspace_root.join("artifacts").join("deploy")
}

pub fn load_node_wallet() -> Result<Keypair, NaclacClientError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| {
            NaclacClientError::General(
                "Could not find HOME or USERPROFILE in environment".to_string(),
            )
        })?;
    let path = std::path::PathBuf::from(home).join(".config/solana/id.json");
    let file_content = std::fs::read_to_string(&path).map_err(|e| {
        NaclacClientError::General(format!(
            "Failed to read Solana wallet keypair at {:?}: {}",
            path, e
        ))
    })?;
    let bytes: Vec<u8> = file_content
        .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n' || c == '\r')
        .split(',')
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    let secret_bytes: [u8; 32] = bytes[0..32].try_into().map_err(|_| {
        NaclacClientError::General(
            "Invalid keypair bytes length, must be at least 32 bytes".to_string(),
        )
    })?;
    Ok(Keypair::new_from_array(secret_bytes))
}

pub fn transfer_sol(
    provider: &NaclacProvider,
    to: &Address,
    lamports: u64,
) -> Result<Signature, NaclacClientError> {
    let payer_address = provider.payer.address();
    let sys_ix = solana_system_interface::instruction::transfer(
        &solana_address::Address::new_from_array(payer_address.to_bytes()),
        &solana_address::Address::new_from_array(to.to_bytes()),
        lamports,
    );
    let ix = to_sdk_instruction(sys_ix);
    let versioned_message = compile_provider_message(provider, &[ix])?;
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer])
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;
    provider
        .send_transaction(&tx, None)
        .map(|meta| meta.signature)
}

pub fn create_mint(
    provider: &NaclacProvider,
    mint_signer: &Keypair,
    mint_authority: &Address,
    decimals: u8,
) -> Result<Signature, NaclacClientError> {
    create_mint_with_program(
        provider,
        mint_signer,
        mint_authority,
        decimals,
        &TOKEN_PROGRAM_ID,
    )
}

pub fn create_mint_with_program(
    provider: &NaclacProvider,
    mint_signer: &Keypair,
    mint_authority: &Address,
    decimals: u8,
    token_program_id: &Address,
) -> Result<Signature, NaclacClientError> {
    let payer_address = provider.payer.address();
    let mint_address = mint_signer.address();

    let space = Mint::LEN;
    let lamports = provider.get_minimum_balance_for_rent_exemption(space)?;

    let sys_ix = solana_system_interface::instruction::create_account(
        &solana_address::Address::new_from_array(payer_address.to_bytes()),
        &solana_address::Address::new_from_array(mint_address.to_bytes()),
        lamports,
        space as u64,
        &solana_address::Address::new_from_array(token_program_id.to_bytes()),
    );
    let create_account_ix = to_sdk_instruction(sys_ix);

    // Construct InitializeMint2 manually
    let mut data = vec![20u8]; // InitializeMint2 discriminant
    data.push(decimals);
    data.extend_from_slice(&mint_authority.to_bytes());
    data.push(0); // freeze_authority_option = None (0)
    data.extend_from_slice(&[0u8; 32]); // freeze_authority dummy

    let init_mint_ix = Instruction {
        program_id: Address::new_from_array(token_program_id.to_bytes()),
        accounts: vec![solana_instruction::AccountMeta::new(
            Address::new_from_array(mint_address.to_bytes()),
            false,
        )],
        data,
    };

    let versioned_message = compile_provider_message(provider, &[create_account_ix, init_mint_ix])?;
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer, mint_signer])
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;

    provider
        .send_transaction(&tx, None)
        .map(|meta| meta.signature)
}

pub fn create_ata(
    provider: &NaclacProvider,
    mint: &Address,
    owner: &Address,
) -> Result<Address, NaclacClientError> {
    create_ata_with_program(provider, mint, owner, &TOKEN_PROGRAM_ID)
}

/// The associated-token-account program's `Create` instruction: `funding`
/// pays for the associated token account of `owner` for `mint` under
/// `token_program_id`, and the instruction fails if that account already
/// exists. The account being created is the instruction's second account.
pub fn create_associated_token_account_instruction(
    funding: &Address,
    owner: &Address,
    mint: &Address,
    token_program_id: &Address,
) -> Instruction {
    let (associated_account, _bump) = Address::find_program_address(
        &[owner.as_ref(), token_program_id.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    Instruction {
        program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
        accounts: vec![
            solana_instruction::AccountMeta::new(*funding, true),
            solana_instruction::AccountMeta::new(associated_account, false),
            solana_instruction::AccountMeta::new_readonly(*owner, false),
            solana_instruction::AccountMeta::new_readonly(*mint, false),
            solana_instruction::AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            solana_instruction::AccountMeta::new_readonly(*token_program_id, false),
        ],
        data: vec![0],
    }
}

pub fn create_ata_with_program(
    provider: &NaclacProvider,
    mint: &Address,
    owner: &Address,
    token_program_id: &Address,
) -> Result<Address, NaclacClientError> {
    let ix = create_associated_token_account_instruction(
        &provider.payer.address(),
        owner,
        mint,
        token_program_id,
    );
    let ata_address = ix.accounts[1].pubkey;

    let versioned_message = compile_provider_message(provider, &[ix])?;
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer])
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;

    provider.send_transaction(&tx, None)?;
    Ok(ata_address)
}

pub fn mint_to(
    provider: &NaclacProvider,
    mint: &Address,
    destination: &Address,
    authority: &Keypair,
    amount: u64,
) -> Result<Signature, NaclacClientError> {
    mint_to_with_program(
        provider,
        mint,
        destination,
        authority,
        amount,
        &TOKEN_PROGRAM_ID,
    )
}

pub fn mint_to_with_program(
    provider: &NaclacProvider,
    mint: &Address,
    destination: &Address,
    authority: &Keypair,
    amount: u64,
    token_program_id: &Address,
) -> Result<Signature, NaclacClientError> {
    let authority_address = authority.address();

    // Construct MintTo manually
    let mut data = Vec::with_capacity(9);
    data.push(7u8); // MintTo discriminant
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: Address::new_from_array(token_program_id.to_bytes()),
        accounts: vec![
            solana_instruction::AccountMeta::new(Address::new_from_array(mint.to_bytes()), false),
            solana_instruction::AccountMeta::new(
                Address::new_from_array(destination.to_bytes()),
                false,
            ),
            solana_instruction::AccountMeta::new_readonly(
                Address::new_from_array(authority_address.to_bytes()),
                true,
            ),
        ],
        data,
    };

    let versioned_message = compile_provider_message(provider, &[ix])?;
    let signers: &[&dyn Signer] = if provider.payer.pubkey() == authority.pubkey() {
        &[&*provider.payer]
    } else {
        &[&*provider.payer, authority]
    };
    let tx = VersionedTransaction::try_new(versioned_message, signers)
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;

    provider
        .send_transaction(&tx, None)
        .map(|meta| meta.signature)
}

pub fn create_token_account(
    provider: &NaclacProvider,
    account_signer: &Keypair,
    mint: &Address,
    owner: &Address,
) -> Result<Signature, NaclacClientError> {
    create_token_account_with_program(provider, account_signer, mint, owner, &TOKEN_PROGRAM_ID)
}

pub fn create_token_account_with_program(
    provider: &NaclacProvider,
    account_signer: &Keypair,
    mint: &Address,
    owner: &Address,
    token_program_id: &Address,
) -> Result<Signature, NaclacClientError> {
    create_token_account_with_program_and_space(
        provider,
        account_signer,
        mint,
        owner,
        token_program_id,
        TokenAccount::LEN,
    )
}

/// Same as `create_token_account_with_program`, but with an explicit
/// account `space` rather than the fixed unextended 165 bytes. Needed for
/// any Token-2022 mint carrying an extension that requires a matching
/// per-account extension (e.g. `TransferFeeConfig` on the mint requires
/// `TransferFeeAmount` space on every token account of that mint) —
/// `InitializeAccount3` fails with `InvalidAccountData` if the account
/// wasn't allocated enough room for the extensions Token-2022 auto-adds.
pub fn create_token_account_with_program_and_space(
    provider: &NaclacProvider,
    account_signer: &Keypair,
    mint: &Address,
    owner: &Address,
    token_program_id: &Address,
    space: usize,
) -> Result<Signature, NaclacClientError> {
    let payer_address = provider.payer.address();
    let account_address = account_signer.address();

    let lamports = provider.get_minimum_balance_for_rent_exemption(space)?;

    let sys_ix = solana_system_interface::instruction::create_account(
        &solana_address::Address::new_from_array(payer_address.to_bytes()),
        &solana_address::Address::new_from_array(account_address.to_bytes()),
        lamports,
        space as u64,
        &solana_address::Address::new_from_array(token_program_id.to_bytes()),
    );
    let create_account_ix = to_sdk_instruction(sys_ix);

    // Construct InitializeAccount3 manually
    let mut data = vec![18u8]; // InitializeAccount3 discriminant
    data.extend_from_slice(&owner.to_bytes());

    let init_account_ix = Instruction {
        program_id: Address::new_from_array(token_program_id.to_bytes()),
        accounts: vec![
            solana_instruction::AccountMeta::new(
                Address::new_from_array(account_address.to_bytes()),
                false,
            ),
            solana_instruction::AccountMeta::new_readonly(
                Address::new_from_array(mint.to_bytes()),
                false,
            ),
        ],
        data,
    };

    let versioned_message =
        compile_provider_message(provider, &[create_account_ix, init_account_ix])?;
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer, account_signer])
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;

    provider
        .send_transaction(&tx, None)
        .map(|meta| meta.signature)
}
