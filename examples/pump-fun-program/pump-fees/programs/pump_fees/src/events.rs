use naclac_lang::prelude::*;
use crate::components::{Fees, FeeTier, Shareholder};

// status: 0 = Paused, 1 = Active (real type is a ConfigStatus enum; bytemuck::Pod
// can't be derived for enums, so this stays a plain u8 — same as in components).

#[event(alloc)]
pub struct CreateFeeSharingConfigEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub pool: Option<Address>,
    pub sharing_config: Address,
    pub admin: Address,
    pub initial_shareholders: Vec<Shareholder>,
    pub status: u8,
}

#[event]
pub struct DonationFeePdaCranked {
    pub timestamp: i64,
    pub amount: u64,
    pub signer: Address,
    pub donation_fee_pda: Address,
    pub config_id: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub creator: Address,
}

#[event]
pub struct DonationFeePdaCreated {
    pub timestamp: i64,
    pub created_by: Address,
    pub donation_fee_pda: Address,
    pub config_id: Address,
    pub base_mint: Address,
    pub quote_mint: Address,
    pub creator: Address,
}

#[event]
pub struct ExtendFeeConfigEvent {
    pub current_size: u64,
    pub new_size: u64,
    pub timestamp: i64,
    pub fee_config: Address,
    pub user: Address,
}

#[event]
pub struct InitializeFeeConfigEvent {
    pub timestamp: i64,
    pub admin: Address,
    pub fee_config: Address,
}

#[event]
pub struct InitializeFeeProgramGlobalEvent {
    pub timestamp: i64,
    pub claim_rate_limit: u64,
    pub authority: Address,
    pub social_claim_authority: Address,
    pub disable_flags: u8,
}

#[event(alloc)]
pub struct ResetFeeSharingConfigEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub sharing_config: Address,
    pub old_admin: Address,
    pub old_shareholders: Vec<Shareholder>,
    pub new_admin: Address,
    pub new_shareholders: Vec<Shareholder>,
    pub old_version: u8,
    pub new_version: u8,
}

#[event]
pub struct SetAuthorityEvent {
    pub timestamp: i64,
    pub old_authority: Address,
    pub new_authority: Address,
}

#[event]
pub struct SetClaimRateLimitEvent {
    pub timestamp: i64,
    pub claim_rate_limit: u64,
}

#[event]
pub struct SetDisableFlagsEvent {
    pub timestamp: i64,
    pub disable_flags: u8,
}

#[event]
pub struct SetSocialClaimAuthorityEvent {
    pub timestamp: i64,
    pub social_claim_authority: Address,
}

// fees-06#14 follow-up (probe6/probe7): the real on-chain event, for both v1
// and v2, is 8 bytes shorter than the IDL's declared 13-field type — no
// `lifetime_stable_claimed` ever appears in the actual emitted bytes.
// Matches real on-chain behavior, not the (apparently stale) IDL type.
#[event(alloc)]
pub struct SocialFeePdaClaimed {
    pub timestamp: i64,
    pub user_id: String,
    pub platform: u8,
    pub social_fee_pda: Address,
    pub recipient: Address,
    pub social_claim_authority: Address,
    pub amount_claimed: u64,
    pub claimable_before: u64,
    pub lifetime_claimed: u64,
    pub recipient_balance_before: u64,
    pub recipient_balance_after: u64,
    pub quote_mint: Address,
}

#[event(alloc)]
pub struct SocialFeePdaCreated {
    pub timestamp: i64,
    pub user_id: String,
    pub platform: u8,
    pub social_fee_pda: Address,
    pub created_by: Address,
}

#[event]
pub struct SweepBuybackEvent {
    pub sol_amount: u64,
    pub token_amount: u64,
    pub destination: Address,
    pub buyback_vault: Address,
    pub mint: Address,
    pub index: u8,
}

#[event]
pub struct UpdateAdminEvent {
    pub timestamp: i64,
    pub old_admin: Address,
    pub new_admin: Address,
}

#[event(alloc)]
pub struct UpdateFeeConfigEvent {
    pub timestamp: i64,
    pub admin: Address,
    pub fee_config: Address,
    pub fee_tiers: Vec<FeeTier>,
    pub flat_fees: Fees,
}

#[event(alloc)]
pub struct UpdateFeeSharesEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub sharing_config: Address,
    pub admin: Address,
    pub new_shareholders: Vec<Shareholder>,
    pub version: u8,
}

#[event(alloc)]
pub struct UpdateStableFeeConfigEvent {
    pub timestamp: i64,
    pub admin: Address,
    pub fee_config: Address,
    pub stable_fee_tiers: Vec<FeeTier>,
    pub flat_fees: Fees,
}

#[event(alloc)]
pub struct UpsertFeeTiersEvent {
    pub timestamp: i64,
    pub admin: Address,
    pub fee_config: Address,
    pub fee_tiers: Vec<FeeTier>,
    pub offset: u8,
}

#[event(alloc)]
pub struct UpsertStableFeeTiersEvent {
    pub timestamp: i64,
    pub admin: Address,
    pub fee_config: Address,
    pub stable_fee_tiers: Vec<FeeTier>,
    pub offset: u8,
}
