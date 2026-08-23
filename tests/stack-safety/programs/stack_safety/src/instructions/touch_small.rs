use naclac_lang::prelude::*;
use crate::components::SmallData;

#[derive(Accounts)]
pub struct TouchSmall {
    #[account(mut)]
    pub small: Account<SmallData>,
}

#[instruction]
pub fn touch_small(ctx: Context<TouchSmall>) -> Result {
    ctx.accounts.small.value += 1;
    Ok(())
}
