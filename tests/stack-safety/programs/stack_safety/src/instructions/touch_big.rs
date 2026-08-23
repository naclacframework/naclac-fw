use naclac_lang::prelude::*;
use crate::components::BigData;

// `big` MUST be boxed here — it's over the per-field stack budget, so this
// is the real reason for the mechanism, not a stand-in for something that
// could just as well stay unboxed. Deref/DerefMut through the Box must
// behave exactly like an unboxed `Account<T>` field.
#[derive(Accounts)]
pub struct TouchBig {
    #[account(mut)]
    pub big: Box<Account<BigData>>,
}

#[instruction]
pub fn touch_big(ctx: Context<TouchBig>) -> Result {
    ctx.accounts.big.payload[0] = ctx.accounts.big.payload[0].wrapping_add(1);
    // `.address()` (ToAddress) must also work transparently through the Box.
    let _addr = ctx.accounts.big.address();
    Ok(())
}
