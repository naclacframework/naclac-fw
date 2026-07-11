use crate::error::NaclacClientError;
use crate::provider::NaclacProvider;
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::{v0, VersionedMessage};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

pub const TOKEN_PROGRAM_ID: Address = Address::new_from_array(spl_token::ID.to_bytes());
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address =
    Address::new_from_array(spl_associated_token_account::ID.to_bytes());
pub const TOKEN_2022_PROGRAM_ID: Address = Address::new_from_array(spl_token_2022::ID.to_bytes());
pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

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
        program_id: Pubkey::new_from_array(ix.program_id.to_bytes()),
        accounts: ix
            .accounts
            .into_iter()
            .map(|meta| solana_program::instruction::AccountMeta {
                pubkey: Pubkey::new_from_array(meta.pubkey.to_bytes()),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: ix.data,
    }
}

pub fn get_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&Sha256::digest(preimage.as_bytes())[0..8]);
    disc
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
    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    let v0_msg =
        v0::Message::try_compile(&payer_pubkey, &[ix], &[], recent_blockhash).map_err(|e| {
            NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e))
        })?;
    let versioned_message = VersionedMessage::V0(v0_msg);
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

    let space = 82; // spl_token::state::Mint::LEN
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
        program_id: Pubkey::new_from_array(token_program_id.to_bytes()),
        accounts: vec![solana_program::instruction::AccountMeta::new(
            Pubkey::new_from_array(mint_address.to_bytes()),
            false,
        )],
        data,
    };

    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    let v0_msg = v0::Message::try_compile(
        &payer_pubkey,
        &[create_account_ix, init_mint_ix],
        &[],
        recent_blockhash,
    )
    .map_err(|e| NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e)))?;
    let versioned_message = VersionedMessage::V0(v0_msg);
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

pub fn create_ata_with_program(
    provider: &NaclacProvider,
    mint: &Address,
    owner: &Address,
    token_program_id: &Address,
) -> Result<Address, NaclacClientError> {
    let payer_address = provider.payer.address();

    // Derive ATA address using find_program_address
    let (ata_address, _) = Address::find_program_address(
        &[owner.as_ref(), token_program_id.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let sys_ix = spl_associated_token_account::instruction::create_associated_token_account(
        &solana_address::Address::new_from_array(payer_address.to_bytes()),
        &solana_address::Address::new_from_array(owner.to_bytes()),
        &solana_address::Address::new_from_array(mint.to_bytes()),
        &solana_address::Address::new_from_array(token_program_id.to_bytes()),
    );
    let ix = to_sdk_instruction(sys_ix);

    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    let v0_msg =
        v0::Message::try_compile(&payer_pubkey, &[ix], &[], recent_blockhash).map_err(|e| {
            NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e))
        })?;
    let versioned_message = VersionedMessage::V0(v0_msg);
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
        program_id: Pubkey::new_from_array(token_program_id.to_bytes()),
        accounts: vec![
            solana_program::instruction::AccountMeta::new(
                Pubkey::new_from_array(mint.to_bytes()),
                false,
            ),
            solana_program::instruction::AccountMeta::new(
                Pubkey::new_from_array(destination.to_bytes()),
                false,
            ),
            solana_program::instruction::AccountMeta::new_readonly(
                Pubkey::new_from_array(authority_address.to_bytes()),
                true,
            ),
        ],
        data,
    };

    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    let v0_msg =
        v0::Message::try_compile(&payer_pubkey, &[ix], &[], recent_blockhash).map_err(|e| {
            NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e))
        })?;
    let versioned_message = VersionedMessage::V0(v0_msg);
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
    let payer_address = provider.payer.address();
    let account_address = account_signer.address();

    let space = 165; // spl_token::state::Account::LEN
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
        program_id: Pubkey::new_from_array(token_program_id.to_bytes()),
        accounts: vec![
            solana_program::instruction::AccountMeta::new(
                Pubkey::new_from_array(account_address.to_bytes()),
                false,
            ),
            solana_program::instruction::AccountMeta::new_readonly(
                Pubkey::new_from_array(mint.to_bytes()),
                false,
            ),
        ],
        data,
    };

    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash()?;
    let v0_msg = v0::Message::try_compile(
        &payer_pubkey,
        &[create_account_ix, init_account_ix],
        &[],
        recent_blockhash,
    )
    .map_err(|e| NaclacClientError::General(format!("Failed to compile v0 message: {:?}", e)))?;
    let versioned_message = VersionedMessage::V0(v0_msg);
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer, account_signer])
        .map_err(|e| NaclacClientError::General(format!("Failed to sign transaction: {:?}", e)))?;

    provider
        .send_transaction(&tx, None)
        .map(|meta| meta.signature)
}
