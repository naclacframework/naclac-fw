//! `Option<...>` account fields over each supported inner wrapper.

use crate::components::{Roles, Tiny, Vault};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct OptionalKinds {
    #[account(mut)]
    pub signer: Signer,
    #[account(mut)]
    pub maybe_vault: Option<Account<Vault>>,
    pub maybe_roles: Option<Account<Roles>>,
    pub maybe_signer: Option<Signer>,
    pub maybe_program: Option<Program<System>>,
    /// SAFETY: optional and never read.
    pub maybe_raw: Option<AccountInfo>,
    #[account(mut, close = signer)]
    pub maybe_closed: Option<Account<Tiny>>,
}

pub fn optional_kinds(_ctx: Context<OptionalKinds>) -> Result {
    Ok(())
}
