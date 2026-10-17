use naclac_lang::prelude::*;

/// Reads a Metaplex Core Collection account via
/// `naclac_metadata::fetch_collection` and asserts its base fields match
/// what's expected — proves `CollectionView`'s byte-offset parser
/// round-trips real on-chain data correctly.
#[derive(Accounts)]
pub struct CheckCollection {
    /// SAFETY: read-only; `fetch_collection` itself validates ownership by
    /// the real Metaplex Core program before parsing any bytes.
    pub collection: AccountInfo,
}

#[instruction_args]
pub struct CheckCollectionArgs {
    pub expected_update_authority: Address,
    pub expected_name: ZcString,
    pub expected_uri: ZcString,
    pub expected_num_minted: u32,
    pub expected_current_size: u32,
}

pub fn check_collection(ctx: Context<CheckCollection>, args: CheckCollectionArgs) -> Result {
    let data = fetch_collection(&ctx.accounts.collection)?;

    if data.update_authority != args.expected_update_authority {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let expected_name_str: &str = &args.expected_name;
    if data.name != expected_name_str {
        return Err(NaclacError::InvalidInstructionData.into());
    }
    let expected_uri_str: &str = &args.expected_uri;
    if data.uri != expected_uri_str {
        return Err(NaclacError::InvalidInstructionData.into());
    }
    if data.num_minted != args.expected_num_minted {
        return Err(NaclacError::InvalidInstructionData.into());
    }
    if data.current_size != args.expected_current_size {
        return Err(NaclacError::InvalidInstructionData.into());
    }

    Ok(())
}
