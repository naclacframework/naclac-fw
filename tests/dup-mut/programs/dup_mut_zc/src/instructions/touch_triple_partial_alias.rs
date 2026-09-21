use naclac_lang::prelude::*;
use crate::components::Vault;

// See `programs/dup_mut/src/instructions/touch_triple_partial_alias.rs` for
// the full rationale — identical shape, solana-zerocopy mode.
#[derive(Accounts)]
pub struct TouchTriplePartialAlias {
    #[account(mut, unsafe(alias))]
    pub a: Account<Vault>,

    #[account(mut, unsafe(alias))]
    pub b: Account<Vault>,

    #[account(mut)]
    pub c: Account<Vault>,
}

pub fn touch_triple_partial_alias(ctx: Context<TouchTriplePartialAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    ctx.accounts.c.balance += 1;
    Ok(())
}
