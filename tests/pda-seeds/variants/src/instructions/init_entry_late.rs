// A PDA whose seed uses the address of `registry`, which is declared after it.
use {
    crate::{
        components::{Entry, Registry},
        constants::SEED_ENTRY_LATE,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitEntryLate {
    #[account(
        init,
        payer = payer,
        seeds = [SEED_ENTRY_LATE, registry.address().as_ref()],
        bump
    )]
    pub entry_late: Account<Entry>,

    pub registry: Account<Registry>,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,
}

pub fn init_entry_late(ctx: Context<InitEntryLate>) -> Result {
    ctx.accounts.entry_late.value = 5;
    Ok(())
}
