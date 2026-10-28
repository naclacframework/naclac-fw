//! `Box<Account<T>>` account fields, which exist only on the Solana backend.

use crate::components::{Registry, Vault};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Boxed {
    pub signer: Signer,
    pub boxed: Box<Account<Vault>>,
    #[account(mut)]
    pub boxed_mut: Box<Account<Registry>>,
}

pub fn boxed(_ctx: Context<Boxed>) -> Result {
    Ok(())
}
