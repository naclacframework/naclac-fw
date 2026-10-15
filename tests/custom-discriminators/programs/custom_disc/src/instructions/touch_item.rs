use naclac_lang::prelude::*;
use crate::components::Item;

// `item` is `mut` with no `seeds`/`address` constraint, so the only thing
// standing between a correctly-typed Item and an arbitrary same-owner
// account (e.g. a Marker) is `Account<Item>`'s discriminator check.
#[derive(Accounts)]
pub struct TouchItem {
    pub caller: Signer,

    #[account(mut)]
    pub item: Account<Item>,
}

pub fn touch_item(ctx: Context<TouchItem>) -> Result {
    ctx.accounts.item.value = ctx.accounts.item.value.wrapping_add(1);
    Ok(())
}
