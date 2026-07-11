use naclac_lang::prelude::*;
use crate::components::Vault;

// Same shape as `touch_pair_no_alias`, but both fields explicitly opt out of
// the duplicate-mutable-account guard via `unsafe(alias)` — the escape
// hatch should let the caller pass the same account for both `a` and `b`.
#[derive(Accounts)]
pub struct TouchPairWithAlias {
    #[account(mut, unsafe(alias))]
    pub a: Account<Vault>,

    #[account(mut, unsafe(alias))]
    pub b: Account<Vault>,
}

#[instruction]
pub fn touch_pair_with_alias(ctx: Context<TouchPairWithAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    Ok(())
}
