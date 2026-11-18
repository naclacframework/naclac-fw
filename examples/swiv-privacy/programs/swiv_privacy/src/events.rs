//! Public protocol configuration and pool settlement events.

use naclac_lang::prelude::*;

#[event]
pub struct ProtocolInitialized {
    pub admin: Address,
    pub fee_wallet: Address,
}

#[event]
pub struct ConfigUpdated {
    pub treasury: Opt<Address>,
    pub protocol_fee_bps: Opt<u64>,
    pub batch_settle_wait_duration: Opt<i64>,
}

#[event]
pub struct PoolCreated {
    pub pool_name: [u8; 128],
    pub pool_name_len: u16,
    pub start_time: i64,
    pub end_time: i64,
}

#[event]
pub struct RewardClaimed {
    pub bet_address: Address,
    pub user: Address,
    pub amount: u64,
}

#[event]
pub struct BetRefunded {
    pub bet_address: Address,
    pub user: Address,
    pub amount: u64,
    pub is_emergency: Bool,
}
