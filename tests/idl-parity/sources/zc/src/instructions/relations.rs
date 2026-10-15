//! Relation constraints (`field = target`, `field = target @ Error`). Two of them are named so the
//! attribute text contains the substrings `init` and `mut` without being those keywords.

use crate::components::{Roles, Vault};
use crate::errors::VaultError;
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Relations {
    #[account(mut, admin = authority)]
    pub vault: Account<Vault>,
    #[account(admin = authority @ VaultError::NotAdmin)]
    pub roles_checked: Account<Roles>,
    #[account(initiator = actor)]
    pub roles_initiator: Account<Roles>,
    #[account(mutator = actor)]
    pub roles_mutator: Account<Roles>,
    pub authority: Signer,
    pub actor: Signer,
}

/// Documented only on the implementation.
pub fn relations(ctx: Context<Relations>) -> Result {
    require!(ctx.accounts.vault.balance == 0, VaultError::Plain);
    Ok(())
}
