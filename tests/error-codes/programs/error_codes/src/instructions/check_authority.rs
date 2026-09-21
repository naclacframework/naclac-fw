use naclac_lang::prelude::*;
use crate::errors::TestError;

#[derive(Accounts)]
pub struct CheckAuthority {
    pub payer: Signer,

    /// SAFETY: only this account's own address is compared against
    /// `payer`'s — its data is never read or deserialized.
    pub required: AccountInfo,
}

pub fn check_authority(ctx: Context<CheckAuthority>) -> Result {
    require!(
        ctx.accounts.payer.address() == ctx.accounts.required.address(),
        TestError::Unauthorized
    );
    Ok(())
}
