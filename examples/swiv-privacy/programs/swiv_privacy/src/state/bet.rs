//! Participant stake, prediction, and settlement entitlement.

use {crate::constants::SEED_BET, naclac_lang::prelude::*};

#[defined_type]
#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum BetStatus {
    Pending,
    Active,
    Resolved,
    Claimed,
}

#[account]
#[seeds(SEED_BET, pool_pubkey, user_pubkey, bump = bump)]
pub struct Bet {
    pub user_pubkey: Address,
    pub pool_pubkey: Address,
    pub stake: u64,
    pub end_timestamp: i64,
    pub creation_ts: i64,
    pub update_count: u32,
    pub calculated_weight: u128,
    pub is_weight_added: Bool,
    pub prediction: u64,
    pub status: ZcEnum<BetStatus>,
    pub bump: u8,
}
