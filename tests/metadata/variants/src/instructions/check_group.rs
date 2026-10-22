use naclac_lang::prelude::*;

/// Reads a `GroupV1` account via `naclac_metadata::fetch_group` and asserts
/// its base fields match what's expected — proves `GroupView`'s byte-offset
/// parser round-trips real on-chain data correctly.
#[derive(Accounts)]
pub struct CheckGroup {
    /// SAFETY: read-only; `fetch_group` itself validates ownership by the
    /// real Metaplex Core program before parsing any bytes.
    pub group: AccountInfo,
}

pub fn check_group(
    ctx: Context<CheckGroup>,
    expected_update_authority: Address,
    expected_name: ZcString,
    expected_uri: ZcString,
) -> Result {
    let data = fetch_group(&ctx.accounts.group)?;

    if data.update_authority != expected_update_authority {
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
