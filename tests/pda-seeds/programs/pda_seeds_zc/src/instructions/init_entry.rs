use naclac_lang::prelude::*;
use crate::components::{Entry, Registry};
use crate::constants::SEED_ENTRY;

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct InitEntry {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_ENTRY, registry.as_ref()],
        bump = bump
    )]
    pub entry: Account<Entry>,

    pub system_program: Program<System>,
}

pub fn init_entry(ctx: Context<InitEntry>, bump: u8) -> Result {
    let entry = &mut ctx.accounts.entry;
    entry.bump = bump;
    entry.value = 0;
    Ok(())
}
