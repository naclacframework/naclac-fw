use naclac_lang::prelude::*;
use crate::state::Counter;
use crate::constants::SEED_COUNTER;
use crate::events::{RosterChanged, RosterChangedAlloc};

#[derive(Accounts)]
pub struct EmitRoster {
    #[account(mut, seeds = [SEED_COUNTER], bump)]
    pub counter: Account<Counter>,
}

pub fn emit_roster(ctx: Context<EmitRoster>) -> Result {
    ctx.accounts.counter.count += 1;

    let mut scores = FixedVec::<u64, 4>::new();
    scores.try_push(10)?;
    scores.try_push(20)?;
    let mut name = FixedString::<16>::new();
    name.try_set("roster")?;

    emit!(RosterChanged { scores, name });
    emit!(RosterChangedAlloc {
        scores,
        name,
        note: String::from("alloc"),
    });

    Ok(())
}
