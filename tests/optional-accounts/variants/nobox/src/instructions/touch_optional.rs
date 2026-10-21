use naclac_lang::prelude::*;
use crate::components::Thing;

// Exercises the sentinel scheme end to end for a single optional field: the
// caller either passes a real `Thing` account (present — incremented) or the
// program's own address as a placeholder (absent — left untouched, no error).
#[derive(Accounts)]
pub struct TouchOptional {
    #[account(mut)]
    pub thing: Option<Account<Thing>>,
}

pub fn touch_optional(ctx: Context<TouchOptional>) -> Result {
    if let Some(thing) = ctx.accounts.thing.as_mut() {
        thing.value += 1;
    }
    Ok(())
}
