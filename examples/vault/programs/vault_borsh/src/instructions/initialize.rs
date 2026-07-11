use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::constants::SEED_VAULT;

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT]
    )]
    pub vault_account: Account<Vault>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn initialize(ctx: Context<Initialize>) -> Result {
    let vault = &mut ctx.accounts.vault_account;

    vault.authority = ctx.accounts.payer.address();
    vault.total_deposited = 0;

    Ok(())
}
