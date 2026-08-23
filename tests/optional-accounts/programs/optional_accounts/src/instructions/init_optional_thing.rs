use naclac_lang::prelude::*;
use crate::components::Thing;
use crate::constants::SEED_OPTIONAL_THING;

// Exercises `init` on a seeded `Option<T>` field: `init_cpi.rs`'s codegen
// runs entirely before `self` exists (on the raw `info`/`accounts[idx]`
// slot), already inside the sentinel's `Some` branch — so a caller passing
// a real, not-yet-existing PDA here creates and initializes it exactly like
// a non-optional `init` would; a caller passing the sentinel skips creation
// entirely, no CPI ever issued.
#[derive(Accounts)]
pub struct InitOptionalThing {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_OPTIONAL_THING]
    )]
    pub optional_thing: Option<Account<Thing>>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_optional_thing(ctx: Context<InitOptionalThing>) -> Result {
    if let Some(optional_thing) = ctx.accounts.optional_thing.as_mut() {
        optional_thing.value = 42;
    }
    Ok(())
}
