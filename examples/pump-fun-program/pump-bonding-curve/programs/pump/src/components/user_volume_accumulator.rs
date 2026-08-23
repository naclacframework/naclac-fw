use naclac_lang::prelude::*;

#[component]
pub struct UserVolumeAccumulator {
    pub user: Address,
    pub needs_claim: Bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub has_total_claimed_tokens: Bool,
    pub cashback_earned: u64,
    pub total_cashback_claimed: u64,
    pub stable_cashback_earned: u64,
    pub total_stable_cashback_claimed: u64,
    pub _reserved_trailing: [u8; 31],
    pub bump: u8,
}
