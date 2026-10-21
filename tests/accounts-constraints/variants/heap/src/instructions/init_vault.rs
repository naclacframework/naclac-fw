use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::constants::SEED_VAULT;

// Covers `init`, `payer`, and `space` together — the space is given
// explicitly (`8 + size_of::<Vault>()`) rather than omitted, specifically to
// exercise that code path (the framework can also auto-compute it, but this
// case is about proving the explicit form works).
#[derive(Accounts)]
pub struct InitVault {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Vault>(),
        seeds = [SEED_VAULT],
        bump
    )]
    pub vault: Account<Vault>,

    pub system_program: Program<System>,
}

pub fn init_vault(ctx: Context<InitVault>) -> Result {
    let vault = &mut ctx.accounts.vault;
    vault.admin = ctx.accounts.payer.address();
    vault.value = 0;
    Ok(())
}
