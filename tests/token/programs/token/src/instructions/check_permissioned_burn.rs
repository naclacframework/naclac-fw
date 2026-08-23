use naclac_lang::prelude::*;

/// Reads the `PermissionedBurnConfig` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the authority
/// matches the expected value — proves the TLV byte-offset math for this
/// extension is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckPermissionedBurn {
    pub mint: InterfaceAccount<Mint>,
}

#[instruction]
pub fn check_permissioned_burn(
    ctx: Context<CheckPermissionedBurn>,
    expected_authority: Option<Address>,
) -> Result {
    let config: PermissionedBurnConfig = ctx.accounts.mint.get_extension()?;

    if config.authority() != expected_authority {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
