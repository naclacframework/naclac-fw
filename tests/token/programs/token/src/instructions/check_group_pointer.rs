use naclac_lang::prelude::*;

/// Reads the `GroupPointer` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the authority and
/// group address match the expected values — the only way to prove the TLV
/// byte-offset math is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckGroupPointer {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_group_pointer(
    ctx: Context<CheckGroupPointer>,
    expected_authority: Option<Address>,
    expected_group_address: Option<Address>,
) -> Result {
    let config: GroupPointer = ctx.accounts.mint.get_extension()?;

    if config.authority() != expected_authority || config.group_address() != expected_group_address
    {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
