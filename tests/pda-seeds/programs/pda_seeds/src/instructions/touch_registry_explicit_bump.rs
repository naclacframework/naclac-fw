use naclac_lang::prelude::*;
use crate::components::Registry;

// Baseline explicit-bump validation on an existing account. The test file
// calls this twice: once with the real bump (must succeed) and once with a
// deliberately wrong bump (must fail with a PDA-mismatch error, not just
// "any error") — confirming the hash-and-compare check actually verifies
// the value rather than accepting anything.
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct TouchRegistryExplicitBump {
    #[account(
        mut,
        seeds = [SEED_REGISTRY],
        bump = bump
    )]
    pub registry: Account<Registry>,
}

#[instruction]
pub fn touch_registry_explicit_bump(ctx: Context<TouchRegistryExplicitBump>, _bump: u8) -> Result {
    let registry = &mut ctx.accounts.registry;
    registry.tag += 1;
    Ok(())
}
