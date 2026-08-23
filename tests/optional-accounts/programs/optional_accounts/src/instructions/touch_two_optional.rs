use naclac_lang::prelude::*;
use crate::components::Thing;

// Two independent optional `mut` fields, both omittable at once — the
// regression case for the `MUT_MASK` fix: two absent slots both carry the
// same sentinel address (the program's own), and must not be mistaken for a
// genuine duplicate-mutable-account collision.
#[derive(Accounts)]
pub struct TouchTwoOptional {
    #[account(mut)]
    pub thing_a: Option<Account<Thing>>,

    #[account(mut)]
    pub thing_b: Option<Account<Thing>>,
}

#[instruction]
pub fn touch_two_optional(ctx: Context<TouchTwoOptional>) -> Result {
    if let Some(thing_a) = ctx.accounts.thing_a.as_mut() {
        thing_a.value += 1;
    }
    if let Some(thing_b) = ctx.accounts.thing_b.as_mut() {
        thing_b.value += 1;
    }
    Ok(())
}
