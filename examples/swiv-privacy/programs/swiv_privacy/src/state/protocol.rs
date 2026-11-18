//! Protocol authority, fee policy, and global pool counter.

use {crate::constants::SEED_PROTOCOL, naclac_lang::prelude::*};

#[account]
#[seeds(SEED_PROTOCOL, bump = bump)]
pub struct Protocol {
    pub admin: Address,
    pub treasury_wallet: Address,
    pub protocol_fee_bps: u64,
    pub paused: Bool,
    pub batch_settle_wait_duration: i64,
    pub total_pools: u64,
    pub bump: u8,
}
