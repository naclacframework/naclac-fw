use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub user: Signer,

    #[account(init, payer = user, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    pub system_program: Program<System>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result {
    let global = &mut ctx.accounts.global;
    global.initialized = true.into();
    global.authority = ctx.accounts.user.address();
    msg!("Global successfully initialized");
    Ok(())
}
