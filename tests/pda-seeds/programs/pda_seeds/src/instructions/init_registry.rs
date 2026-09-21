use naclac_lang::prelude::*;
use crate::components::Registry;
use crate::constants::SEED_REGISTRY;

// Single-literal seed -> compile-time-precomputed PDA path. Bare `bump`
// (no explicit value) is allowed here specifically because the seed is
// fully resolvable at macro-expansion time; it also makes the framework
// auto-write the derived bump into `registry.bump` after init (see
// naclac-macros/src/accounts.rs's `mut_zero_copy_fields` handling), so
// later instructions can read it back without us doing it manually.
#[derive(Accounts)]
pub struct InitRegistry {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_REGISTRY],
        bump
    )]
    pub registry: Account<Registry>,

    pub system_program: Program<System>,
}

pub fn init_registry(ctx: Context<InitRegistry>) -> Result {
    let registry = &mut ctx.accounts.registry;
    registry.tag = 0;
    registry.label = [0, 0, 0, 0];
    Ok(())
}
