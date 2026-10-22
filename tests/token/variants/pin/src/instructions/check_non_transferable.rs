use naclac_lang::prelude::*;

/// Reads the `NonTransferable` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts its presence
/// matches `expected_present` — proves the TLV byte-offset math for this
/// extension is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckNonTransferable {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_non_transferable(ctx: Context<CheckNonTransferable>, expected_present: u8) -> Result {
    let present = ctx.accounts.mint.get_extension::<NonTransferable>().is_ok();

    if present != (expected_present != 0) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
