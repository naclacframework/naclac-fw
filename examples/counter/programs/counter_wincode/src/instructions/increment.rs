use {
    crate::{
        errors::CounterError,
        events::CounterIncremented,
        state::counter::Counter,
        systems::math::process_increment,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Increment {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        seeds = Counter::seeds(),
        authority = authority @ CounterError::Unauthorized
    )]
    pub counter_account: Account<Counter>,
}

pub fn increment(ctx: Context<Increment>) -> Result {
    let counter_account = &mut ctx.accounts.counter_account;

    let new_count = process_increment(counter_account)?;

    emit!(CounterIncremented {
        new_count,
        timestamp: unix_timestamp()?,
    });

    Ok(())
}
