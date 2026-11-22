//! Members approve a proposal by signing the transaction; their accounts are
//! the instruction's remaining accounts.

use {
    crate::{
        errors::TreasuryError,
        state::{Proposal, Treasury},
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Approve {
    pub treasury: Account<Treasury>,

    #[account(mut, treasury = treasury)]
    pub proposal: Account<Proposal>,
}

/// Records every remaining account as an approval. Fails when one is not a
/// signer, is not a member, has already approved, or would exceed the member
/// limit.
pub fn approve(ctx: Context<Approve>) -> Result {
    let treasury = &ctx.accounts.treasury;
    let proposal = &mut ctx.accounts.proposal;
    require!(!bool::from(proposal.executed), TreasuryError::AlreadyExecuted);

    for account in ctx.remaining_accounts.iter() {
        require!(account.is_signer(), ProgramError::MissingRequiredSignature);
        let member = account.address();
        require!(treasury.members.contains(&member), TreasuryError::NotAMember);
        require!(
            !proposal.approvals.contains(&member),
            TreasuryError::AlreadyApproved
        );
        proposal.approvals.try_push(member)?;
    }
    Ok(())
}
