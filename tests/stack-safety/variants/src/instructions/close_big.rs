use naclac_lang::prelude::*;
use crate::components::BigData;

// `close`'s lamport-drain/reassign logic (`close_account.rs`) operates on
// the field's `AccountInfo` — this is the ToAccountInfo/AsRefByteSlice side
// of the boxed-field blanket impls, not just Deref/DerefMut on the inner
// data like `touch_big` exercises.
#[derive(Accounts)]
pub struct CloseBig {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, close = payer)]
    pub big: Box<Account<BigData>>,
}

pub fn close_big(_ctx: Context<CloseBig>) -> Result {
    Ok(())
}
