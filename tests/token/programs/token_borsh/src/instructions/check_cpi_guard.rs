use naclac_lang::prelude::*;

/// Reads the `CpiGuard` extension off a Token-2022 token account via
/// `InterfaceAccount<TokenAccount>::get_extension` and asserts both its
/// presence and its `lock_cpi` flag match the expected values — proves the
/// TLV byte-offset math for this extension is correct end-to-end, not just
/// that it compiles.
#[derive(Accounts)]
pub struct CheckCpiGuard {
    pub vault: InterfaceAccount<TokenAccount>,
}

#[instruction]
pub fn check_cpi_guard(
    ctx: Context<CheckCpiGuard>,
    expected_present: u8,
    expected_locked: u8,
) -> Result {
    match ctx.accounts.vault.get_extension::<CpiGuard>() {
        Ok(ext) => {
            if expected_present == 0 {
                return Err(NaclacError::ConstraintAddress.into());
            }
            if ext.lock_cpi() != (expected_locked != 0) {
                return Err(NaclacError::ConstraintAddress.into());
            }
        }
        Err(_) => {
            if expected_present != 0 {
                return Err(NaclacError::ConstraintAddress.into());
            }
        }
    }

    Ok(())
}
