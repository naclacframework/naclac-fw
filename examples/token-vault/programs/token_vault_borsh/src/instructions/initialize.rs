use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::constants::SEED_VAULT;

#[derive(Accounts)]
#[instruction(vault_id: u64, vault_bump: u8)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT, &vault_id.to_le_bytes()],
        bump = vault_bump
    )]
    pub vault_account: Account<Vault>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn initialize(ctx: Context<Initialize>, vault_id: u64, vault_bump: u8) -> Result {
    let vault = &mut ctx.accounts.vault_account;

    vault.authority = ctx.accounts.payer.address();
    vault.vault_id = vault_id;
    vault.bump = vault_bump;

    Ok(())
}
