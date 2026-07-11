use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::constants::SEED_VAULT_A;

#[derive(Accounts)]
pub struct InitVaultA {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT_A]
    )]
    pub vault_a: Account<Vault>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_vault_a(ctx: Context<InitVaultA>) -> Result {
    let vault_a = &mut ctx.accounts.vault_a;
    vault_a.balance = 0;
    Ok(())
}
