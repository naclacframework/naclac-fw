use naclac_lang::prelude::*;
use crate::components::Vault;

// `close`: drains the target account's lamports to `destination`, zeroes
// its data (discriminator included), and reassigns it to the System
// Program — so a later instruction in the same transaction (or a later
// transaction) that tries to load it as a `Vault` again is rejected by the
// ordinary owner/discriminator checks, not a dedicated "closed" sentinel.
#[derive(Accounts)]
pub struct CloseVault {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, close = payer)]
    pub vault: Account<Vault>,
}

pub fn close_vault(_ctx: Context<CloseVault>) -> Result {
    Ok(())
}
