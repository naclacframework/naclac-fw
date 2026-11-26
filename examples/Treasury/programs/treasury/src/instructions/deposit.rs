//! Moves lamports from a depositor into a treasury's vault.

use {
    crate::{
        errors::TreasuryError,
        events::Deposited,
        state::{Treasury, VaultPda},
        types::Status,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub depositor: Signer,

    #[account(mut)]
    pub treasury: Account<Treasury>,

    /// SAFETY: a lamport-only PDA with no data, checked by `seeds` against the
    /// treasury's stored vault bump, which only `create_treasury` writes, from
    /// the canonical search; it is only the destination of a transfer.
    #[account(
        mut,
        seeds = VaultPda::seeds(treasury.address()),
        bump = treasury.vault_bump
    )]
    pub vault: UncheckedAccount,

    pub system_program: Program<System>,
}

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result {
    require!(amount > 0, TreasuryError::ZeroAmount);
    require!(
        ctx.accounts.treasury.status.get()? == Status::Active,
        TreasuryError::Paused
    );

    ctx.accounts.system_program.transfer(
        SystemTransferAccounts {
            from: &mut ctx.accounts.depositor,
            to: &mut ctx.accounts.vault,
        },
        amount,
    )?;

    let treasury = &mut ctx.accounts.treasury;
    treasury.total_deposited = treasury
        .total_deposited
        .checked_add(amount)
        .ok_or(TreasuryError::Overflow)?;

    emit!(Deposited {
        depositor: ctx.accounts.depositor.address(),
        amount,
    });
    Ok(())
}
