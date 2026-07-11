use naclac_lang::prelude::*;
use crate::components::Config;
use crate::constants::SEED_CONFIG;

#[derive(Accounts)]
pub struct InitConfig {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_CONFIG]
    )]
    pub config: Account<Config>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_config(ctx: Context<InitConfig>) -> Result {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.payer.address();
    config.fee_bps = 100;
    Ok(())
}
