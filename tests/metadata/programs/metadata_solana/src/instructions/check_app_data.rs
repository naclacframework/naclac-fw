use naclac_lang::prelude::*;

/// Reads an Asset's AppData adapter via `naclac_metadata::fetch_asset_app_data`
/// and asserts the stored bytes equal `b"hello"` (what `write_app_data`
/// writes) — proves the external-registry walk and the adapter's
/// offset/length-pair data read are both correct end-to-end.
#[derive(Accounts)]
pub struct CheckAppData {
    /// SAFETY: read-only; `fetch_asset_app_data` itself validates ownership
    /// by the real Metaplex Core program before parsing any bytes.
    pub asset: AccountInfo,
}

pub fn check_app_data(ctx: Context<CheckAppData>) -> Result {
    let info = fetch_asset_app_data(&ctx.accounts.asset, PluginAuthorityArg::UpdateAuthority)?
        .ok_or(NaclacError::InvalidInstructionData)?;

    #[cfg(not(feature = "pinocchio"))]
    let bytes: &[u8] = &info.data;
    #[cfg(feature = "pinocchio")]
    let bytes: &[u8] = info.data.as_bytes();

    if bytes != b"hello" {
        return Err(NaclacError::InvalidInstructionData.into());
    }

    Ok(())
}
