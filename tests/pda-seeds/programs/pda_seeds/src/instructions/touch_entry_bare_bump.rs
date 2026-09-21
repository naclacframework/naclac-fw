use naclac_lang::prelude::*;
use crate::components::{Entry, Registry};
use crate::constants::SEED_ENTRY;

// The specific gap ZERO_COPY_CONSISTENCY_PLAN.md Part 6e flagged as
// completely untested: a DYNAMIC (non-literal) seed combined with bare
// `bump` on an EXISTING (non-`init`) zero-copy account. This forces the
// macro through its "read the stored bump back out of the account's own
// data at runtime" path (`field_ident.bump`, security.rs's dynamic-PDA
// branch) rather than the compile-time-precomputed path `init_registry`
// exercises.
#[derive(Accounts)]
pub struct TouchEntryBareBump {
    pub registry: Account<Registry>,

    #[account(
        mut,
        seeds = [SEED_ENTRY, registry.as_ref()],
        bump
    )]
    pub entry: Account<Entry>,
}

pub fn touch_entry_bare_bump(ctx: Context<TouchEntryBareBump>) -> Result {
    let entry = &mut ctx.accounts.entry;
    entry.value += 1;
    Ok(())
}
