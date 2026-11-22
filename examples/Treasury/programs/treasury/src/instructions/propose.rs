//! A member proposes paying lamports out of the vault. The proposal's id is the
//! treasury's next id, and a note passed by the proposer is counted.

use {
    crate::{
        errors::TreasuryError,
        state::{Note, Proposal, Treasury},
        types::Status,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Propose {
    #[account(mut)]
    pub proposer: Signer,

    #[account(mut)]
    pub treasury: Account<Treasury>,

    #[account(
        init,
        payer = proposer,
        seeds = Proposal::seeds(treasury.address(), id),
        bump
    )]
    pub proposal: Account<Proposal>,

    #[account(mut)]
    pub note: Option<Account<Note>>,

    pub system_program: Program<System>,
}

pub fn propose(
    ctx: Context<Propose>,
    id: u64,
    recipient: Address,
    amount: u64,
    deadline: i64,
) -> Result {
    let proposer = ctx.accounts.proposer.address();

    let treasury = &mut ctx.accounts.treasury;
    require!(treasury.status.get()? == Status::Active, TreasuryError::Paused);
    require!(treasury.members.contains(&proposer), TreasuryError::NotAMember);
    require!(id == treasury.next_id, TreasuryError::InvalidProposalId);
    require!(amount > 0, TreasuryError::ZeroAmount);
    require!(deadline > unix_timestamp()?, TreasuryError::DeadlinePassed);
    treasury.next_id = id.checked_add(1).ok_or(TreasuryError::Overflow)?;

    let proposal = &mut ctx.accounts.proposal;
    proposal.treasury = treasury.address();
    proposal.id = id;
    proposal.proposer = proposer;
    proposal.recipient = recipient;
    proposal.amount = amount;
    proposal.deadline = deadline;
    proposal.bump = ctx.bumps.proposal;

    if let Some(note) = ctx.accounts.note.as_mut() {
        require!(note.authority == proposer, TreasuryError::NotNoteAuthority);
        note.count = note.count.checked_add(1).ok_or(TreasuryError::Overflow)?;
    }
    Ok(())
}
