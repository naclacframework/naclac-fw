use naclac_lang::prelude::*;
use crate::components::{Entry, Registry};
use crate::constants::SEED_ENTRY;

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

#[instruction]
pub fn touch_entry_bare_bump(ctx: Context<TouchEntryBareBump>) -> Result {
    let entry = &mut ctx.accounts.entry;
    entry.value += 1;
    Ok(())
}
