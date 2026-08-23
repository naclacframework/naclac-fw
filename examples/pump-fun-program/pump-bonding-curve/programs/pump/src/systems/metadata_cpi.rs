use naclac_lang::prelude::*;
use crate::constants::{MINT_AUTHORITY_SEED, MPL_TOKEN_METADATA_PROGRAM_ID};

/// Metaplex's own single-byte instruction-enum discriminator for
/// `CreateMetadataAccountV3` (index 33), confirmed against the real
/// `mpl-token-metadata` crate source (`generated/instructions/
/// create_metadata_account_v3.rs`) — distinct from naclac's own 8-byte
/// sighash discriminators.
const CREATE_METADATA_ACCOUNT_V3_DISCRIMINATOR: u8 = 33;

pub struct CreateMetadataCpiAccounts {
    pub metadata: AccountView,
    pub mint: AccountView,
    pub mint_authority: AccountView,
    pub payer: AccountView,
    pub system_program: AccountView,
}

/// Hand-built CPI into `mpl_token_metadata::CreateMetadataAccountV3`, since
/// naclac has no generated client for foreign programs and the real
/// `mpl-token-metadata` crate's own instruction builders aren't used here —
/// this manually Borsh-encodes the same `DataV2`/args layout the real crate
/// does (verified from its source, not guessed), with `creators`/
/// `collection`/`uses`/`collection_details` all `None` and `is_mutable:
/// true`. `update_authority` is set to the same `mint_authority` PDA that
/// signs the mint, appearing twice in the account list (once as
/// `mint_authority`, once as `update_authority`) — both are the same real
/// account, which Solana's CPI accounts list allows.
pub fn create_metadata_via_cpi(
    accounts: CreateMetadataCpiAccounts,
    name: &str,
    symbol: &str,
    uri: &str,
    mint_authority_bump: u8,
) -> Result<()> {
    let mut ix_data = Vec::new();
    ix_data.push(CREATE_METADATA_ACCOUNT_V3_DISCRIMINATOR);
    for s in [name, symbol, uri] {
        ix_data.extend_from_slice(&(s.len() as u32).to_le_bytes());
        ix_data.extend_from_slice(s.as_bytes());
    }
    ix_data.extend_from_slice(&0u16.to_le_bytes()); // seller_fee_basis_points
    ix_data.push(0); // creators: None
    ix_data.push(0); // collection: None
    ix_data.push(0); // uses: None
    ix_data.push(1); // is_mutable: true
    ix_data.push(0); // collection_details: None

    let CreateMetadataCpiAccounts { metadata, mint, mint_authority, payer, system_program } = accounts;

    let ix_accounts = [
        instruction::InstructionAccount::writable(metadata.address()),
        instruction::InstructionAccount::readonly(mint.address()),
        instruction::InstructionAccount::readonly_signer(mint_authority.address()),
        instruction::InstructionAccount::writable_signer(payer.address()),
        instruction::InstructionAccount::readonly_signer(mint_authority.address()),
        instruction::InstructionAccount::readonly(system_program.address()),
    ];

    let ix = instruction::InstructionView {
        program_id: MPL_TOKEN_METADATA_PROGRAM_ID.as_address(),
        accounts: &ix_accounts,
        data: &ix_data,
    };

    let account_infos = [metadata, mint, mint_authority, payer, mint_authority, system_program];
    let signer_seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, &[mint_authority_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    invoke_signed_pinocchio(&ix, &account_infos, signer)
}

/// Reads the first entry (`creators[0].address`) out of a real Metaplex
/// `Metadata` account's Borsh-encoded `creators: Option<Vec<Creator>>` field,
/// matching the real `mpl-token-metadata` crate's layout (confirmed against
/// its actual source at the pinned tag `mpl-token-metadata-v5.1.2-alpha.2`):
/// `key(1) + update_authority(32) + mint(32) + name/symbol/uri(String, u32
/// len-prefixed) + seller_fee_basis_points(u16) + creators`, with
/// `Creator { address(32), verified(1), share(1) }`. Returns `None` for
/// `creators = None`, an empty `creators` vec, or data too short to contain
/// a complete, well-formed prefix up to that point — real pump.so's own
/// `Account<Metadata>` deserialization would already have rejected a
/// genuinely malformed account before reaching this logic, so a short buffer
/// here only happens for an account this framework doesn't itself validate
/// as strictly; treating it as "no creators" is the fail-safe choice (worst
/// case a creator sync is skipped, never a wrong creator written).
pub fn read_first_metaplex_creator(data: &[u8]) -> Option<Address> {
    let mut offset = 1 + 32 + 32; // key + update_authority + mint
    for _ in 0..3 {
        // name, symbol, uri
        let len = u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize;
        offset += 4 + len;
    }
    offset += 2; // seller_fee_basis_points
    let has_creators = *data.get(offset)?;
    offset += 1;
    if has_creators == 0 {
        return None;
    }
    let count = u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize;
    offset += 4;
    if count == 0 {
        return None;
    }
    let address_bytes = data.get(offset..offset + 32)?;
    Some(Address::new_from_array(address_bytes.try_into().ok()?))
}
