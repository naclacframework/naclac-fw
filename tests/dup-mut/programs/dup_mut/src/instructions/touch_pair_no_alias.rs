use naclac_lang::prelude::*;
use crate::components::Vault;

// No `unsafe(alias)` on either field, and deliberately no `seeds`/`address`
// constraint (so the caller can pass any two account addresses, including
// the same one twice) — this isolates the duplicate-mutable-account guard
// itself: the compile-time `MUT_MASK` bitmask checked against the runtime
// `__duplicates` bitvec in `accounts.rs`'s generated `load_and_validate`.
#[derive(Accounts)]
pub struct TouchPairNoAlias {
    #[account(mut)]
    pub a: Account<Vault>,

    #[account(mut)]
    pub b: Account<Vault>,
}

pub fn touch_pair_no_alias(ctx: Context<TouchPairNoAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    Ok(())
}
