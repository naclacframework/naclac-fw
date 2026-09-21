use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::constants::SEED_VAULT_B;

#[derive(Accounts)]
pub struct InitVaultB {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT_B]
    )]
    pub vault_b: Account<Vault>,

    pub system_program: Program<System>,
}

pub fn init_vault_b(ctx: Context<InitVaultB>) -> Result {
    let vault_b = &mut ctx.accounts.vault_b;
    vault_b.balance = 0;
    Ok(())
}
