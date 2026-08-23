use naclac_lang::prelude::*;

// No `bump` field — confirmed absent from the real account (fees-02-accounts-and-state.md).
#[component]
pub struct BuybackVault {
    pub authority: Address,
    pub total_claimed: u64,
    pub total_claimed_token1: u64,
    pub total_claimed_token2: u64,
    pub last_claimed: i64,
    pub claim_rate_limit: i64,
    pub _reserved: [u8; 128],
}
