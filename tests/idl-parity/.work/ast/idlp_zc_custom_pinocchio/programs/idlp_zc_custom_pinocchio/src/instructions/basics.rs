//! Every account wrapper type that has no external-program account type behind it, and the IDL
//! flags derived from the wrapper alone.

use crate::components::{Roles, Vault};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct WrapperKinds {
    /// A plain signer.
    pub signer: Signer,
    /// Mutable component account.
    #[account(mut)]
    pub vault: Account<Vault>,
    pub roles: Account<Roles>,
    /// SAFETY: unchecked and never read.
    pub raw: AccountInfo,
    /// SAFETY: unchecked and never read; its name starts with an underscore.
    pub _spare: AccountInfo,
    pub system_program: Program<System>,
    pub sys: Program<System>,
    pub token_program: Program<Token>,
    pub token_2022_program: Program<Token2022>,
    pub associated_token_program: Program<AssociatedToken>,
    pub token_interface: Interface<TokenInterface>,
}

/// Implementation-side documentation that differs from the wrapper's.
pub fn wrapper_kinds(_ctx: Context<WrapperKinds>) -> Result {
    Ok(())
}
