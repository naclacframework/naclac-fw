use naclac_lang::prelude::*;

/// Reads a Collection's MasterEdition plugin via
/// `naclac_metadata::fetch_collection_master_edition` and asserts its
/// `max_supply` matches what's expected — proves the plugin's own variable-
/// width payload read (two leading `Option<String>` fields before it) is
/// correct end-to-end.
#[derive(Accounts)]
pub struct CheckMasterEdition {
    /// SAFETY: read-only; `fetch_collection_master_edition` itself
    /// validates ownership by the real Metaplex Core program before parsing
    /// any bytes.
    pub collection: AccountInfo,
}

pub fn check_master_edition(
    ctx: Context<CheckMasterEdition>,
    expected_max_supply: u32,
) -> Result {
    let info = fetch_collection_master_edition(&ctx.accounts.collection)?
        .ok_or(NaclacError::InvalidInstructionData)?;

    if info.max_supply != Some(expected_max_supply) {
        return Err(NaclacError::InvalidInstructionData.into());
    }

    Ok(())
}
