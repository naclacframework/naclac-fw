use naclac_lang::prelude::*;

// `rent_exempt`: bare-flag constraint (no value, like `executable`) that
// rejects an account whose lamport balance is below the rent-exempt minimum
// for its current data length — `Rent::get()?.is_exempt(lamports, data_len)`
// on non-pinocchio, a const-rent formula on pinocchio (`security.rs`'s
// "Rent-Exemption Check"). Bare `AccountInfo` since only lamports/data-len
// metadata is inspected, never the account's contents.
#[derive(Accounts)]
pub struct CheckRentExempt {
    /// SAFETY: only this account's lamport balance and data length are
    /// inspected via the `rent_exempt` constraint below; its data is never
    /// read or deserialized.
    #[account(rent_exempt)]
    pub target: AccountInfo,
}

pub fn check_rent_exempt(_ctx: Context<CheckRentExempt>) -> Result {
    Ok(())
}
