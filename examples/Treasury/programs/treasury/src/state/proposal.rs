//! A proposal to pay lamports out of a treasury's vault.

use {
    crate::constants::{MAX_MEMBERS, SEED_PROPOSAL},
    naclac_lang::prelude::*,
};

#[account(discriminator = 2)]
#[seeds(SEED_PROPOSAL, treasury, id, bump = bump)]
pub struct Proposal {
    pub treasury: Address,
    pub id: u64,
    pub proposer: Address,
    pub recipient: Address,
    pub amount: u64,
    pub deadline: i64,
    pub executed: Bool,
    pub bump: u8,
    pub approvals: FixedVec<Address, MAX_MEMBERS>,
}
