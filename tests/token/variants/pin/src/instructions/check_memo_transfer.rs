use naclac_lang::prelude::*;

/// Reads the `MemoTransfer` extension off a Token-2022 token account via
/// `InterfaceAccount<TokenAccount>::get_extension` and asserts both its
/// presence and its `require_incoming_transfer_memos` flag match the
/// expected values — proves the TLV byte-offset math for this extension is
/// correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckMemoTransfer {
    pub vault: InterfaceAccount<TokenAccount>,
}

pub fn check_memo_transfer(
    ctx: Context<CheckMemoTransfer>,
    expected_present: u8,
    expected_required: u8,
) -> Result {
    match ctx.accounts.vault.get_extension::<MemoTransfer>() {
        Ok(ext) => {
            if expected_present == 0 {
                return Err(NaclacError::ConstraintAddress.into());
            }
            if ext.require_incoming_transfer_memos() != (expected_required != 0) {
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
