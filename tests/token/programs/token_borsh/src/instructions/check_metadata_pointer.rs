use naclac_lang::prelude::*;

/// Reads the `MetadataPointer` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the authority and
/// metadata address match the expected values — the only way to prove the
/// TLV byte-offset math is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckMetadataPointer {
    pub mint: InterfaceAccount<Mint>,
}

#[instruction]
pub fn check_metadata_pointer(
    ctx: Context<CheckMetadataPointer>,
    expected_authority: Option<Address>,
    expected_metadata_address: Option<Address>,
) -> Result {
    let config: MetadataPointer = ctx.accounts.mint.get_extension()?;

    if config.authority() != expected_authority
        || config.metadata_address() != expected_metadata_address
    {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
