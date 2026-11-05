use naclac_lang::prelude::*;

// Account layout for the duplicate-mutable-account tests: `first` and
// `second` are `mut` at slots 1 and 2, `authority` (slot 0) and `observer`
// (slot 3) are read-only. Passing the same address in two of these slots
// makes the framework reject the instruction, and the error's slot index must
// be the `mut` account's slot.
#[derive(Accounts)]
pub struct CheckDuplicateMut {
    pub authority: Signer,

    /// SAFETY: never read or written; only its position and writable flag
    /// matter to the duplicate check.
    #[account(mut)]
    pub first: AccountInfo,

    /// SAFETY: never read or written; only its position and writable flag
    /// matter to the duplicate check.
    #[account(mut)]
    pub second: AccountInfo,

    /// SAFETY: never read or written; only its position matters to the
    /// duplicate check.
    pub observer: AccountInfo,
}

pub fn check_duplicate_mut(_ctx: Context<CheckDuplicateMut>) -> Result {
    Ok(())
}
