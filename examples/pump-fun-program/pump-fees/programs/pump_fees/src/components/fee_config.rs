use naclac_lang::prelude::*;
use crate::constants::MAX_FEE_TIERS;

#[derive(Default)]
#[cfg_attr(feature = "debug-mode", derive(Debug))]
#[naclac_pod]
pub struct Fees {
    pub lp_fee_bps: u64,
    pub protocol_fee_bps: u64,
    pub creator_fee_bps: u64,
}

#[cfg_attr(feature = "debug-mode", derive(Debug))]
#[naclac_pod]
pub struct FeeTier {
    pub market_cap_lamports_threshold: u64,
    pub fees: Fees,
}

// Fields ordered so no 8-byte-aligned field ever follows a non-multiple-of-8
// offset (avoids an internal padding gap between fields, which a tightly-packed
// TS decoder can't skip over — trailing padding at the struct's end is fine).
#[component]
pub struct FeeConfig {
    /// The flat fees for non-pump pools
    pub flat_fees: Fees,
    /// The fee tiers
    pub fee_tiers: [FeeTier; MAX_FEE_TIERS],
    /// The fee tiers
    pub stable_fee_tiers: [FeeTier; MAX_FEE_TIERS],
    pub fee_tiers_len: u32,
    pub stable_fee_tiers_len: u32,
    /// The bump for the PDA
    pub bump: u8,
    /// The admin account that can update the fee config
    pub admin: Address,
}
