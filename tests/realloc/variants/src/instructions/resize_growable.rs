use naclac_lang::prelude::*;
use crate::components::Growable;
use crate::constants::SEED_GROWABLE;

// No existing example in this repo uses `realloc` at all (confirmed while
// researching this case) — this is the first real exercise of
// `realloc.rs`'s codegen, growing and shrinking, with the account's own
// stored `bump` used to re-derive its seeds (proven pattern from
// `pda-seeds`' `touch_entry_bare_bump`).
#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct ResizeGrowable {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        mut,
        seeds = [SEED_GROWABLE],
        bump,
        realloc = new_space as usize,
        realloc::payer = payer,
        realloc::zero = true
    )]
    pub growable: Account<Growable>,

    pub system_program: Program<System>,
}

pub fn resize_growable(_ctx: Context<ResizeGrowable>, _new_space: u64) -> Result {
    Ok(())
}
