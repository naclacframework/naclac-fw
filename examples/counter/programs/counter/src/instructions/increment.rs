use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::systems::math::process_increment;
use crate::events::CounterIncremented;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct Increment {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        seeds = [SEED_COUNTER], 
        authority = authority
    )]
    pub counter_account: Account<Counter>,
}

#[instruction]
pub fn increment(ctx: Context<Increment>) -> Result {
    let counter_account = &mut ctx.accounts.counter_account;

    let new_count = process_increment(counter_account)?;

    emit!(CounterIncremented {
        new_count,
        timestamp: unix_timestamp()?,
    });

    Ok(())
}
