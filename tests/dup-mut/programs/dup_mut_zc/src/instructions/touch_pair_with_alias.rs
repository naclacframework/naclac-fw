use naclac_lang::prelude::*;
use crate::components::Vault;

#[derive(Accounts)]
pub struct TouchPairWithAlias {
    #[account(mut, unsafe(alias))]
    pub a: Account<Vault>,

    #[account(mut, unsafe(alias))]
    pub b: Account<Vault>,
}

pub fn touch_pair_with_alias(ctx: Context<TouchPairWithAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    Ok(())
}
