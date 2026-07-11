use naclac_lang::prelude::*;
use crate::components::Vault;

#[derive(Accounts)]
pub struct TouchPairNoAlias {
    #[account(mut)]
    pub a: Account<Vault>,

    #[account(mut)]
    pub b: Account<Vault>,
}

#[instruction]
pub fn touch_pair_no_alias(ctx: Context<TouchPairNoAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    Ok(())
}
