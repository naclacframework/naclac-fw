use naclac_lang::prelude::*;

/// Reads the `PausableConfig` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts both its authority
/// and `paused` flag match the expected values — proves the TLV byte-offset
/// math for this extension is correct end-to-end, not just that it
/// compiles.
#[derive(Accounts)]
pub struct CheckPausableConfig {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_pausable_config(
    ctx: Context<CheckPausableConfig>,
    expected_authority: Option<Address>,
    expected_paused: u8,
) -> Result {
    let config: PausableConfig = ctx.accounts.mint.get_extension()?;

    if config.authority() != expected_authority {
        return Err(NaclacError::ConstraintAddress.into());
    }
    if config.paused() != (expected_paused != 0) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
