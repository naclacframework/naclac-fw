//! Closes an executed proposal and refunds its rent to the proposer.

use {
    crate::{errors::TreasuryError, state::Proposal},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CloseProposal {
    #[account(mut)]
    pub proposer: Signer,

    #[account(mut, close = proposer, proposer = proposer)]
    pub proposal: Account<Proposal>,
}

pub fn close_proposal(ctx: Context<CloseProposal>) -> Result {
    require!(
        bool::from(ctx.accounts.proposal.executed),
        TreasuryError::NotExecuted
    );
    Ok(())
}
