use naclac_lang::prelude::*;
use crate::components::Registry;
use crate::constants::SEED_REGISTRY;

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

pub fn touch_registry_explicit_bump(ctx: Context<TouchRegistryExplicitBump>, _bump: u8) -> Result {
    let registry = &mut ctx.accounts.registry;
    registry.tag += 1;
    Ok(())
}
