use naclac_lang::prelude::*;

/// Reads a Metaplex Core Asset account via `naclac_metadata::fetch_asset`
/// and asserts its base fields match what's expected — proves the
/// `AssetView` byte-offset parser round-trips real on-chain data correctly,
/// not just that it compiles.
#[derive(Accounts)]
pub struct CheckAsset {
    /// SAFETY: read-only; `fetch_asset` itself validates ownership by the
    /// real Metaplex Core program before parsing any bytes.
    pub asset: AccountInfo,
}

pub fn check_asset(
    ctx: Context<CheckAsset>,
    expected_owner: Address,
    expected_update_authority: Address,
    expected_name: ZcString,
    expected_uri: ZcString,
) -> Result {
    let data = fetch_asset(&ctx.accounts.asset)?;

    if data.owner != expected_owner {
        return Err(NaclacError::ConstraintAddress.into());
    }
    if data.update_authority != UpdateAuthorityKind::Address(expected_update_authority) {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let expected_name_str: &str = &expected_name;
    if data.name != expected_name_str {
        return Err(NaclacError::InvalidInstructionData.into());
    }
    let expected_uri_str: &str = &expected_uri;
    if data.uri != expected_uri_str {
        return Err(NaclacError::InvalidInstructionData.into());
    }

    Ok(())
}
