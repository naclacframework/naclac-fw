//! Pool configuration, lifecycle, and settlement accounting.

use {
    crate::constants::{MAX_TITLE_LENGTH, SEED_POOL},
    naclac_lang::prelude::*,
};

#[defined_type]
#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum PoolStatus {
    Upcoming,
    Active,
    Closed,
    Resolving,
    Resolved,
    Settled,
    Cancelled,
}

#[account]
#[seeds(SEED_POOL, created_by, pool_id: u64, bump = bump)]
pub struct Pool {
    pub created_by: Address,
    pub title: FixedString<MAX_TITLE_LENGTH>,
    pub pool_id: u64,
    pub stake_token_mint: Address,
    pub start_time: i64,
    pub end_time: i64,
    pub cutoff_time: i64,
    pub total_staked: u64,
    pub distributable_amount: u64,
    pub max_accuracy_buffer: u64,
    pub conviction_bonus_bps: u64,
    pub resolution_result: u64,
    pub resolution_ts: i64,
    pub total_weight: u128,
    pub total_participants: u64,
    pub remaining_participants: u64,
    pub weights_calculated_count: u64,
    pub status: ZcEnum<PoolStatus>,
    pub bump: u8,
}
