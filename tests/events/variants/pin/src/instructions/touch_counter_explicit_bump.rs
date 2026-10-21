use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;

// Explicit `bump = counter.bump` — a self-reference on an *existing*
// (non-`init`) account, never actually exercised by any test in this repo
// before now. Reasoned (not just assumed) to work identically to bare
// `bump`'s auto-path: `security.rs`'s auto-bump branch for a mut zero-copy
// existing field (the one `pda-seeds`' `touch_entry_bare_bump` proves)
// synthesizes exactly `#field_ident.bump` as the bump expression — writing
// `bump = counter.bump` by hand just supplies that same expression
// ourselves instead of having the macro generate it. Verifying that
// reasoning for real rather than leaving it as an assumption.
#[derive(Accounts)]
pub struct TouchCounterExplicitBump {
    #[account(mut, seeds = [SEED_COUNTER], bump = counter.bump)]
    pub counter: Account<Counter>,
}

pub fn touch_counter_explicit_bump(ctx: Context<TouchCounterExplicitBump>) -> Result {
    ctx.accounts.counter.count += 1;
    Ok(())
}
