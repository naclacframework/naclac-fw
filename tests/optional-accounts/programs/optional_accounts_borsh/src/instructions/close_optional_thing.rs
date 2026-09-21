use naclac_lang::prelude::*;
use crate::components::Thing;

// Exercises `close` on an `Option<T>` field: `close_account.rs`'s codegen
// runs in `teardown()`, after `self` exists, so unlike `init` it needed a
// real fix — the close logic is wrapped in `if let Some(__target) = ...`
// and simply skipped when the field is absent (nothing to close).
#[derive(Accounts)]
pub struct CloseOptionalThing {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, close = payer)]
    pub optional_thing: Option<Account<Thing>>,
}

pub fn close_optional_thing(_ctx: Context<CloseOptionalThing>) -> Result {
    Ok(())
}
