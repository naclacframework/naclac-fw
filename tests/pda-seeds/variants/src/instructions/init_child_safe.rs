use naclac_lang::prelude::*;
use crate::state::{Child, Registry};
use crate::constants::SEED_CHILD_SAFE;

/// Same dynamic seed shape as `init_child` (`registry.bump.to_le_bytes().as_ref()`),
/// under its own seed prefix.
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
