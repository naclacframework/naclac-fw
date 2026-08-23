use naclac_lang::prelude::*;

/// Reads the `ImmutableOwner` extension off a Token-2022 token account via
/// `InterfaceAccount<TokenAccount>::get_extension` and asserts its presence
/// matches `expected_present` — proves the TLV byte-offset math for this
/// extension is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckImmutableOwner {
    pub vault: InterfaceAccount<TokenAccount>,
}

#[instruction]
pub fn check_immutable_owner(ctx: Context<CheckImmutableOwner>, expected_present: u8) -> Result {
    let present = ctx.accounts.vault.get_extension::<ImmutableOwner>().is_ok();

    if present != (expected_present != 0) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
