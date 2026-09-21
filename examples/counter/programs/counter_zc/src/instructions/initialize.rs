use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_COUNTER],
    )]
    pub counter_account: Account<Counter>,

    pub system_program: Program<System>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result {

    let counter = &mut ctx.accounts.counter_account;

    counter.authority = ctx.accounts.payer.address();
    counter.count = 0;

    Ok(())
}
