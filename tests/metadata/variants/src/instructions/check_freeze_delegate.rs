use naclac_lang::prelude::*;

/// Reads an Asset's FreezeDelegate plugin via
/// `naclac_metadata::fetch_asset_freeze_delegate` and asserts its `frozen`
/// state matches what's expected — proves `find_plugin_offset`'s registry
/// walk and the plugin's own payload read are both correct end-to-end.
#[derive(Accounts)]
pub struct CheckFreezeDelegate {
    /// SAFETY: read-only; `fetch_asset_freeze_delegate` itself validates
    /// ownership by the real Metaplex Core program before parsing any bytes.
    pub asset: AccountInfo,
}

pub fn check_freeze_delegate(ctx: Context<CheckFreezeDelegate>, expected_frozen: Bool) -> Result {
    let expected_frozen: bool = expected_frozen.into();
    let frozen = fetch_asset_freeze_delegate(&ctx.accounts.asset)?
        .ok_or(NaclacError::AccountNotInitialized)?;

    if frozen != expected_frozen {
        return Err(NaclacError::InvalidInstructionData.into());
    }

    Ok(())
}
