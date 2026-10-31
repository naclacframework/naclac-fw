// A PDA declared before the accounts its constraints name: its seed reads
// `registry`'s data and its `init` payer is `payer`, both declared after it.
use {
    crate::{
        state::{Child, Registry},
        constants::SEED_CHILD_LATE,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitChildLate {
    #[account(
        init,
        payer = payer,
        seeds = [SEED_CHILD_LATE, registry.bump.to_le_bytes().as_ref()],
        bump
    )]
    pub child_late: Account<Child>,

    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    pub system_program: Program<System>,
}

pub fn init_child_late(ctx: Context<InitChildLate>) -> Result {
    ctx.accounts.child_late.value = 9;
    Ok(())
}
