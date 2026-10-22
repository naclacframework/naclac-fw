use naclac_lang::prelude::*;

#[instruction_args]
pub struct CheckInterestBearingMintArgs {
    pub expected_rate_authority: Option<Address>,
    pub expected_current_rate: i16,
}

/// Reads the `InterestBearingConfig` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the rate authority
/// and current rate match the expected values — the only way to prove the
/// TLV byte-offset math is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
#[instruction(args: CheckInterestBearingMintArgs)]
pub struct CheckInterestBearingMint {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_interest_bearing_mint(
    ctx: Context<CheckInterestBearingMint>,
    args: CheckInterestBearingMintArgs,
) -> Result {
    let config: InterestBearingConfig = ctx.accounts.mint.get_extension()?;

    let matches = config.rate_authority() == args.expected_rate_authority
        && config.current_rate() == args.expected_current_rate;

    if !matches {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
