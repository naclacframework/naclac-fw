//! Moves a count from one of an authority's notes to another. Passing the same
//! note twice is rejected before the handler runs.

use {
    crate::{errors::TreasuryError, state::Note},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct MergeNotes {
    pub authority: Signer,

    #[account(mut, authority = authority)]
    pub from: Account<Note>,

    #[account(mut, authority = authority)]
    pub to: Account<Note>,
}

pub fn merge_notes(ctx: Context<MergeNotes>, amount: u64) -> Result {
    let from = &mut ctx.accounts.from;
    let to = &mut ctx.accounts.to;
    require!(from.count >= amount, TreasuryError::InsufficientCount);
    from.count -= amount;
    to.count = to.count.checked_add(amount).ok_or(TreasuryError::Overflow)?;
    Ok(())
}
