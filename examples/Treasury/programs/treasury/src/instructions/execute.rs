//! Pays an approved proposal out of the vault, signing as the vault's PDA, and
//! returns what the vault still holds.

use {
    crate::{
        errors::TreasuryError,
        events::ProposalExecuted,
        state::{Proposal, Treasury, VaultPda},
        types::Status,
    },
    naclac_lang::prelude::*,
};

#[event_cpi]
#[derive(Accounts)]
pub struct Execute {
    pub executor: Signer,

    #[account(mut)]
    pub treasury: Account<Treasury>,

    #[account(mut, treasury = treasury, recipient = recipient)]
    pub proposal: Account<Proposal>,

    /// SAFETY: a lamport-only PDA with no data, checked by `seeds` against the
    /// treasury's stored vault bump, which only `create_treasury` writes, from
    /// the canonical search; it is only the source of a signed transfer.
    #[account(
        mut,
        seeds = VaultPda::seeds(treasury.address()),
        bump = treasury.vault_bump
    )]
    pub vault: UncheckedAccount,

    /// SAFETY: only the destination of lamports, checked against the
    /// proposal's recipient.
    #[account(mut)]
    pub recipient: UncheckedAccount,

    pub system_program: Program<System>,
}

pub fn execute(ctx: Context<Execute>) -> Result<u64> {
    let treasury_address = ctx.accounts.treasury.address();
    let vault_bump = [ctx.accounts.treasury.vault_bump];

    let treasury = &mut ctx.accounts.treasury;
    let proposal = &mut ctx.accounts.proposal;
    require!(treasury.status.get()? == Status::Active, TreasuryError::Paused);
    require!(!bool::from(proposal.executed), TreasuryError::AlreadyExecuted);
    require!(
        proposal.approvals.len() >= usize::from(treasury.threshold),
        TreasuryError::NotEnoughApprovals
    );
    require!(
        unix_timestamp()? <= proposal.deadline,
        TreasuryError::DeadlinePassed
    );

    let amount = proposal.amount;
    treasury.total_deposited = treasury
        .total_deposited
        .checked_sub(amount)
        .ok_or(TreasuryError::InsufficientFunds)?;
    proposal.executed = Bool::new(true);
    let id = proposal.id;
    let recipient = proposal.recipient;

    let vault_seeds = VaultPda::seeds(&treasury_address);
    let signer_seeds = vault_seeds.signer_seeds(&vault_bump);
    ctx.accounts.system_program.transfer_signed(
        SystemTransferAccounts {
            from: &mut ctx.accounts.vault,
            to: &mut ctx.accounts.recipient,
        },
        amount,
        &[signer_seeds.signer()],
    )?;

    emit_cpi!(ProposalExecuted {
        id,
        amount,
        recipient,
    });

    ctx.accounts.vault.lamports()
}
