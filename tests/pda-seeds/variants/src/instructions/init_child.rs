use naclac_lang::prelude::*;
use crate::components::{Child, Registry};
use crate::constants::SEED_CHILD;

// `registry.bump.to_le_bytes().as_ref()` — a struct-field access
// (`registry.bump`, reachable via `Account<T>: Deref<Target = T>`)
// followed by a method-call suffix chain (`.to_le_bytes()`, `.as_ref()`).
// Distinct from `init_entry`'s whole-account `.as_ref()` case: this exercises
// the generic seed-normalization fallback's handling of a nested
// field-then-method expression, not the "is this actually a method call on
// the whole account" classification finding #3 was about.
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct InitChild {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_CHILD, registry.bump.to_le_bytes().as_ref()],
        bump = bump
    )]
    pub child: Account<Child>,

    pub system_program: Program<System>,
}

pub fn init_child(ctx: Context<InitChild>, bump: u8) -> Result {
    let child = &mut ctx.accounts.child;
    child.bump = bump;
    child.value = 0;
    Ok(())
}
