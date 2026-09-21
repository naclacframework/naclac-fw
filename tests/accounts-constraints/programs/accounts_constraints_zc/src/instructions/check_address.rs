use naclac_lang::prelude::*;

// `address`: rejects an account whose own key doesn't match the given
// expression — distinct from `owner` (which checks the *owning program*,
// not the account's own key).
#[derive(Accounts)]
pub struct CheckAddress {
    /// SAFETY: only the account's own address is inspected via the
    /// `address` constraint below; its data is never read or deserialized.
    #[account(address = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub target: AccountInfo,
}

pub fn check_address(_ctx: Context<CheckAddress>) -> Result {
    Ok(())
}
