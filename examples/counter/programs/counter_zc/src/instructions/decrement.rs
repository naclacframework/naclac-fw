use {
    crate::{
        components::counter::Counter,
        constants::SEED_COUNTER,
        events::CounterDecremented,
        systems::math::process_decrement,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Decrement {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        seeds = [SEED_COUNTER],
        authority = authority
    )]
    pub counter_account: Account<Counter>,
}

pub fn decrement(ctx: Context<Decrement>) -> Result {
    let counter_account = &mut ctx.accounts.counter_account;

    let new_count = process_decrement(counter_account)?;

    emit!(CounterDecremented {
        new_count,
        timestamp: unix_timestamp()?,
    });

    Ok(())
}
