//! The treasury account and the seeds of its lamport-only vault.

use {
    crate::{
        constants::{MAX_MEMBERS, SEED_TREASURY, SEED_VAULT},
        types::Status,
    },
    naclac_lang::prelude::*,
};

#[account(discriminator = 1)]
#[seeds(SEED_TREASURY, admin, bump = bump)]
pub struct Treasury {
    pub admin: Address,
    pub bump: u8,
    pub vault_bump: u8,
    pub threshold: u8,
    pub status: ZcEnum<Status>,
    pub next_id: u64,
    pub total_deposited: u64,
    pub members: FixedVec<Address, MAX_MEMBERS>,
}

#[derive(Seeds)]
#[seeds(SEED_VAULT, treasury: Address)]
pub struct VaultPda;
