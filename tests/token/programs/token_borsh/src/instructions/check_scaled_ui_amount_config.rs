use naclac_lang::prelude::*;

/// Reads the `ScaledUiAmountConfig` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the authority and
/// current multiplier match the expected values — proves the TLV
/// byte-offset math for this extension is correct end-to-end, not just
/// that it compiles. `expected_multiplier_bits` carries the expected `f64`
/// as raw `u64` bits over the wire, same reasoning as
/// `create_mint2022_with_scaled_ui_amount.rs`.
#[derive(Accounts)]
pub struct CheckScaledUiAmountConfig {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_scaled_ui_amount_config(
    ctx: Context<CheckScaledUiAmountConfig>,
    expected_authority: Option<Address>,
    expected_multiplier_bits: u64,
) -> Result {
    let config: ScaledUiAmountConfig = ctx.accounts.mint.get_extension()?;
    let expected_multiplier = f64::from_bits(expected_multiplier_bits);

    if config.authority() != expected_authority {
        return Err(NaclacError::ConstraintAddress.into());
    }
    if config.multiplier() != expected_multiplier {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
