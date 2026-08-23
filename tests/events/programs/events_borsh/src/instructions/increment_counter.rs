use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;
use crate::events::CounterIncremented;

#[derive(Accounts)]
pub struct IncrementCounter {
    // Bare `bump` on an existing (non-`init`) zero-copy field reads the
    // account's own already-loaded `.bump` back automatically — the exact,
    // proven shape `tests/pda-seeds/`'s `touch_entry_bare_bump` already
    // covers. See `touch_counter_explicit_bump.rs` for the explicit
    // `bump = counter.bump` self-reference form, verified separately.
    #[account(mut, seeds = [SEED_COUNTER], bump)]
    pub counter: Account<Counter>,
}

#[instruction]
pub fn increment_counter(ctx: Context<IncrementCounter>) -> Result {
    ctx.accounts.counter.count += 1;
    let new_count = ctx.accounts.counter.count;

    emit!(CounterIncremented { new_count });

    Ok(())
}
