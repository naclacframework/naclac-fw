use naclac_lang::prelude::*;

/// Reads the `MintCloseAuthority` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the close authority
/// matches the expected value — proves the TLV byte-offset math for this
/// extension is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckMintCloseAuthority {
    pub mint: InterfaceAccount<Mint>,
}

#[instruction]
pub fn check_mint_close_authority(
    ctx: Context<CheckMintCloseAuthority>,
    expected_close_authority: Option<Address>,
) -> Result {
    let config: MintCloseAuthority = ctx.accounts.mint.get_extension()?;

    if config.close_authority() != expected_close_authority {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
