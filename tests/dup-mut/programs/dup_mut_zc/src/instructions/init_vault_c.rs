use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::constants::SEED_VAULT_C;

// Third independent vault, added specifically for the 3-mut-slot pairwise
// aliasing case (`touch_triple_partial_alias`) — `vault_a`/`vault_b` alone
// only ever exercise a single aliased pair, never a mix of an aliased pair
// plus a genuinely distinct third mutable slot in the same instruction.
#[derive(Accounts)]
pub struct InitVaultC {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT_C]
    )]
    pub vault_c: Account<Vault>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_vault_c(ctx: Context<InitVaultC>) -> Result {
    let vault_c = &mut ctx.accounts.vault_c;
    vault_c.balance = 0;
    Ok(())
}
