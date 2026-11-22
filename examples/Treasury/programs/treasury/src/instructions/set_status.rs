//! Pauses or resumes a treasury; only its admin may.

use {
    crate::{errors::TreasuryError, state::Treasury, types::Status},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetStatus {
    pub admin: Signer,

    #[account(mut, admin = admin)]
    pub treasury: Account<Treasury>,
}

pub fn set_status(ctx: Context<SetStatus>, status: Status) -> Result {
    let treasury = &mut ctx.accounts.treasury;
    require!(treasury.status.get()? != status, TreasuryError::SameStatus);
    treasury.status.set(status);
    Ok(())
}
