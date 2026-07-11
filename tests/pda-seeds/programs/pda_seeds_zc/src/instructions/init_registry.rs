use naclac_lang::prelude::*;
use crate::components::Registry;
use crate::constants::SEED_REGISTRY;

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

#[instruction]
pub fn init_registry(ctx: Context<InitRegistry>) -> Result {
    let registry = &mut ctx.accounts.registry;
    registry.tag = 0;
    registry.label = [0, 0, 0, 0];
    Ok(())
}
