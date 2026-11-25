//! Creates a treasury for an admin and records the bump of its vault.

use {
    crate::{
        constants::MAX_MEMBERS,
        errors::TreasuryError,
        events::TreasuryCreated,
        state::{Treasury, VaultPda},
        types::Status,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(vault_bump: u8)]
pub struct CreateTreasury {
    #[account(mut)]
    pub admin: Signer,

    #[account(
        init,
        payer = admin,
        seeds = Treasury::seeds(admin.address()),
        bump
    )]
    pub treasury: Account<Treasury>,

    /// SAFETY: a lamport-only PDA with no data, checked by `seeds` and the
    /// canonical bump the caller supplies; it is only the source of signed
    /// transfers.
    #[account(seeds = VaultPda::seeds(treasury.address()), bump = vault_bump)]
    pub vault: AccountInfo,

    pub system_program: Program<System>,
}

pub fn create_treasury(ctx: Context<CreateTreasury>, vault_bump: u8, threshold: u8) -> Result {
    require!(
        threshold > 0 && usize::from(threshold) <= MAX_MEMBERS,
        TreasuryError::InvalidThreshold
    );

    let treasury = &mut ctx.accounts.treasury;
    treasury.admin = ctx.accounts.admin.address();
    treasury.bump = ctx.bumps.treasury;
    treasury.vault_bump = vault_bump;
    treasury.threshold = threshold;
    treasury.status.set(Status::Active);

    emit!(TreasuryCreated {
        admin: ctx.accounts.admin.address(),
        threshold,
    });
    Ok(())
}
