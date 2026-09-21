use naclac_lang::prelude::*;
use crate::components::{Child, Registry};
use crate::constants::SEED_CHILD_SAFE;

/// Same dynamic seed shape as `init_child` (`registry.bump.to_le_bytes().as_ref()`),
/// but bare `bump` instead of a client-supplied `bump = <expr>` — no bump
/// argument at all. Exercises the fix for naclac's dynamic-seed-`init`
/// canonicalization gap: previously a hard `compile_error!`
/// (naclac-macros/src/instruction/security.rs's `Some(PdaBump::Auto)` arm
/// reached when `init_config.is_some()`), now routed through
/// `naclac_lang::prelude::find_program_address`'s real on-chain canonical
/// search instead of trusting an unverified client-supplied bump.
#[derive(Accounts)]
pub struct InitChildSafe {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_CHILD_SAFE, registry.bump.to_le_bytes().as_ref()],
        bump
    )]
    pub child_safe: Account<Child>,

    pub system_program: Program<System>,
}

pub fn init_child_safe(ctx: Context<InitChildSafe>) -> Result {
    let child_safe = &mut ctx.accounts.child_safe;
    child_safe.bump = ctx.bumps.child_safe;
    child_safe.value = 0;
    Ok(())
}
