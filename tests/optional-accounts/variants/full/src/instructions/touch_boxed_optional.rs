use naclac_lang::prelude::*;
use crate::components::Thing;

// Proves `Option<Box<Account<Thing>>>` ("Box inside Option") already works
// via naclac-core's existing blanket `NaclacAccount`/`ToAddress`/
// `ToAccountInfo` impls for `Box<T>` — no macro changes needed, unlike
// `Box<Option<T>>` (Box outside), which `type_classify.rs` still doesn't
// detect as optional at all and would need real, more invasive work. Only
// meaningful where the wrapper holds decoded data (Borsh, Wincode, and
// zero-copy on the solana runtime): zero-copy `Account<T>` on pinocchio is
// already pointer-sized and has no `Box<T>` impls, so that mode builds the
// `nobox` family, which omits this instruction.
#[derive(Accounts)]
pub struct TouchBoxedOptional {
    #[account(mut)]
    pub thing: Option<Box<Account<Thing>>>,
}

pub fn touch_boxed_optional(ctx: Context<TouchBoxedOptional>) -> Result {
    if let Some(thing) = ctx.accounts.thing.as_mut() {
        thing.value += 1;
    }
    Ok(())
}
