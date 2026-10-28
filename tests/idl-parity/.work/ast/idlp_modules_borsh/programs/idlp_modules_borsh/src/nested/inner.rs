//! A file module two levels down: an event and an accounts struct whose seeds use constants
//! from two other modules.

use crate::flat::FLAT_SEED;
use crate::nested::NESTED_SEED;
use naclac_lang::prelude::*;

#[event]
pub struct InnerEvent {
    pub id: u32,
}

#[derive(Accounts)]
pub struct NestedAccounts {
    pub signer: Signer,
    /// SAFETY: only its address is checked against the seeds.
    #[account(seeds = [NESTED_SEED, FLAT_SEED], bump)]
    pub pda: AccountInfo,
}
