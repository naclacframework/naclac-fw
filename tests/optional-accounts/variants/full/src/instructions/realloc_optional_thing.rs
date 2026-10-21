use naclac_lang::prelude::*;
use crate::components::Thing;

// Exercises `realloc` on an `Option<T>` field: same shape of fix as `close`
// (runs in `teardown()`, after `self` exists) — wrapped in
// `if let Some(__target) = self.field.as_mut() { ... }`, skipped entirely
// when absent (nothing to resize).
#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct ReallocOptionalThing {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        mut,
        realloc = new_space as usize,
        realloc::payer = payer,
        realloc::zero = true
    )]
    pub optional_thing: Option<Account<Thing>>,

    pub system_program: Program<System>,
}

pub fn realloc_optional_thing(_ctx: Context<ReallocOptionalThing>, _new_space: u64) -> Result {
    Ok(())
}
