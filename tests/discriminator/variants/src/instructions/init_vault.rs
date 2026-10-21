use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::constants::SEED_VAULT;

#[derive(Accounts)]
pub struct InitVault {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT]
    )]
    pub vault: Account<Vault>,

    pub system_program: Program<System>,
}

pub fn init_vault(ctx: Context<InitVault>) -> Result {
    let vault = &mut ctx.accounts.vault;
    vault.owner = ctx.accounts.payer.address();
    vault.balance = 0;
    Ok(())
}
