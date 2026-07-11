use naclac_lang::prelude::*;

// `owner`: rejects an account whose owning program doesn't match the given
// expression. Uses a bare `AccountInfo` since we only ever inspect the
// account's owner metadata, never its data.
#[derive(Accounts)]
pub struct CheckOwner {
    /// SAFETY: only the account's owner field is inspected via the `owner`
    /// constraint below; its data is never read or deserialized.
    #[account(owner = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub target: AccountInfo,
}

#[instruction]
pub fn check_owner(_ctx: Context<CheckOwner>) -> Result {
    Ok(())
}
