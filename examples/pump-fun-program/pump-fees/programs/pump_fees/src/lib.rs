#![no_std]
use naclac_lang::prelude::*;

declare_id!("cBPbCAyFhnUXngDY23SoKwvPrS5ppDaS1VjakEegoDV");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod systems;

use events::{DonationFeePdaCranked, SocialFeePdaClaimed};
use instructions::*;
use state::{Fees, FeeTier, Shareholder};

#[program]
pub mod pump_fees {
    /// Get Fees
    pub fn get_fees(
        ctx: Context<GetFees>,
        is_pump_pool: Bool,
        market_cap_lamports: u128,
        trade_size_lamports: u64,
        is_new_quote_mint: Bool,
    ) -> Result<Fees> {
        get_fees::get_fees(ctx, is_pump_pool, market_cap_lamports, trade_size_lamports, is_new_quote_mint)
    }

    /// Initialize FeeConfig admin
    pub fn initialize_fee_config(ctx: Context<InitializeFeeConfig>, fee_config_bump: u8) -> Result {
        initialize_fee_config::initialize_fee_config(ctx, fee_config_bump)
    }

    pub fn create_fee_sharing_config(
        ctx: Context<CreateFeeSharingConfig>,
        bonding_curve_bump: u8,
        sharing_config_bump: u8,
    ) -> Result {
        create_fee_sharing_config::create_fee_sharing_config(ctx, bonding_curve_bump, sharing_config_bump)
    }

    pub fn create_donation_fee_pda(
        ctx: Context<CreateDonationFeePda>,
        bonding_curve_bump: u8,
        pool_authority_bump: u8,
        pool_bump: u8,
        donation_fee_pda_bump: u8,
    ) -> Result {
        create_donation_fee_pda::create_donation_fee_pda(
            ctx,
            bonding_curve_bump,
            pool_authority_bump,
            pool_bump,
            donation_fee_pda_bump,
        )
    }

    pub fn crank_donation_fee_pda(
        ctx: Context<CrankDonationFeePda>,
        args: CrankDonationFeePdaArgs,
    ) -> Result<Option<DonationFeePdaCranked>> {
        crank_donation_fee_pda::crank_donation_fee_pda(ctx, args)
    }

    pub fn initialize_fee_program_global(
        ctx: Context<InitializeFeeProgramGlobal>,
        social_claim_authority: Address,
        disable_flags: u8,
        claim_rate_limit: u64,
    ) -> Result {
        initialize_fee_program_global::initialize_fee_program_global(
            ctx,
            social_claim_authority,
            disable_flags,
            claim_rate_limit,
        )
    }

    /// Update admin (only callable by admin)
    pub fn update_admin(ctx: Context<UpdateAdmin>) -> Result {
        update_admin::update_admin(ctx)
    }

    /// Realloc the `fee_config` PDA to `FeeConfig::CURRENT_SIZE`. Callable by
    /// any signer (`user` pays the rent delta, per the real program's IDL —
    /// there's no admin/ownership check on this instruction).
    pub fn extend_fee_config(ctx: Context<ExtendFeeConfig>) -> Result {
        extend_fee_config::extend_fee_config(ctx)
    }

    /// Set/Replace fee parameters entirely (only callable by admin)
    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        fee_tiers: ZcVec<FeeTier>,
        flat_fees: Fees,
    ) -> Result {
        update_fee_config::update_fee_config(ctx, fee_tiers, flat_fees)
    }

    /// Set/Replace fee parameters entirely (only callable by admin)
    pub fn update_stable_fee_config(
        ctx: Context<UpdateStableFeeConfig>,
        stable_fee_tiers: ZcVec<FeeTier>,
    ) -> Result {
        update_stable_fee_config::update_stable_fee_config(ctx, stable_fee_tiers)
    }

    /// Update or expand fee tiers (only callable by admin)
    pub fn upsert_fee_tiers(
        ctx: Context<UpsertFeeTiers>,
        fee_tiers: ZcVec<FeeTier>,
        offset: u8,
    ) -> Result {
        upsert_fee_tiers::upsert_fee_tiers(ctx, fee_tiers, offset)
    }

    /// Update or expand fee tiers (only callable by admin)
    pub fn upsert_stable_fee_tiers(
        ctx: Context<UpsertStableFeeTiers>,
        stable_fee_tiers: ZcVec<FeeTier>,
        offset: u8,
    ) -> Result {
        upsert_stable_fee_tiers::upsert_stable_fee_tiers(ctx, stable_fee_tiers, offset)
    }

    pub fn set_authority(ctx: Context<SetAuthority>, new_authority: Address) -> Result {
        set_authority::set_authority(ctx, new_authority)
    }

    pub fn set_claim_rate_limit(ctx: Context<SetClaimRateLimit>, claim_rate_limit: u64) -> Result {
        set_claim_rate_limit::set_claim_rate_limit(ctx, claim_rate_limit)
    }

    pub fn set_disable_flags(ctx: Context<SetDisableFlags>, disable_flags: u8) -> Result {
        set_disable_flags::set_disable_flags(ctx, disable_flags)
    }

    pub fn set_social_claim_authority(
        ctx: Context<SetSocialClaimAuthority>,
        social_claim_authority: Address,
    ) -> Result {
        set_social_claim_authority::set_social_claim_authority(ctx, social_claim_authority)
    }

    pub fn initialize_buyback(
        ctx: Context<InitializeBuyback>,
        index: u8,
        buyback_vault_bump: u8,
    ) -> Result {
        initialize_buyback::initialize_buyback(ctx, index, buyback_vault_bump)
    }

    pub fn sweep_buyback(
        ctx: Context<SweepBuyback>,
        index: u8,
        buyback_vault_bump: u8,
    ) -> Result {
        sweep_buyback::sweep_buyback(ctx, index, buyback_vault_bump)
    }

    /// Gated by the *global* `fee_program_global.authority`, not the vault's own
    /// `authority` — an intentional admin-override design (fees-05).
    pub fn update_buyback_authority(
        ctx: Context<UpdateBuybackAuthority>,
        index: u8,
        buyback_vault_bump: u8,
        new_authority: Address,
    ) -> Result {
        update_buyback_authority::update_buyback_authority(ctx, index, buyback_vault_bump, new_authority)
    }

    /// Gated by the *global* `fee_program_global.authority`, not the vault's own
    /// `authority` — an intentional admin-override design (fees-05).
    pub fn update_buyback_claim_rate_limit(
        ctx: Context<UpdateBuybackClaimRateLimit>,
        index: u8,
        buyback_vault_bump: u8,
        claim_rate_limit: i64,
    ) -> Result {
        update_buyback_claim_rate_limit::update_buyback_claim_rate_limit(
            ctx,
            index,
            buyback_vault_bump,
            claim_rate_limit,
        )
    }

    /// Revoke Fee Sharing Authority
    ///
    /// confirmed via direct execution against the real bytecode —
    /// this instruction always reverts with DeprecatedInstruction, checked
    /// before touching any accounts.
    pub fn revoke_fee_sharing_authority(ctx: Context<RevokeFeeSharingAuthority>) -> Result {
        revoke_fee_sharing_authority::revoke_fee_sharing_authority(ctx)
    }

    /// Transfer Fee Sharing Authority
    ///
    /// confirmed via direct execution against the real bytecode —
    /// this instruction always reverts with DeprecatedInstruction, checked
    /// before touching any accounts.
    pub fn transfer_fee_sharing_authority(ctx: Context<TransferFeeSharingAuthority>) -> Result {
        transfer_fee_sharing_authority::transfer_fee_sharing_authority(ctx)
    }

    pub fn create_social_fee_pda(
        ctx: Context<CreateSocialFeePda>,
        user_id: ZcString,
        platform: u8,
        social_fee_pda_bump: u8,
    ) -> Result {
        create_social_fee_pda::create_social_fee_pda(ctx, user_id, platform, social_fee_pda_bump)
    }

    pub fn claim_social_fee_pda(
        ctx: Context<ClaimSocialFeePda>,
        user_id: ZcString,
        platform: u8,
    ) -> Result<Option<SocialFeePdaClaimed>> {
        claim_social_fee_pda::claim_social_fee_pda(ctx, user_id, platform)
    }

    pub fn claim_social_fee_pda_v2(
        ctx: Context<ClaimSocialFeePdaV2>,
        user_id: ZcString,
        platform: u8,
    ) -> Result<Option<SocialFeePdaClaimed>> {
        claim_social_fee_pda_v2::claim_social_fee_pda_v2(ctx, user_id, platform)
    }

    /// Reset Fee Sharing Config and distribute pending fees first. Sweeps any AMM-side
    /// coin-creator fees into the bonding-curve vault, distributes the vault's full
    /// pending balance to the *current* shareholder list (one `remaining_accounts`
    /// entry per shareholder, in order), then overwrites `sharing_config` with a single
    /// 100%-share `new_admin` entry and bumps its version to 2 — confirmed end-to-end
    /// via `reference/fee-tier-probe/src/bin/probe11.rs` against the real bytecode.
    pub fn reset_fee_sharing_config(
        ctx: Context<ResetFeeSharingConfig>,
        bonding_curve_bump: u8,
        pump_creator_vault_bump: u8,
        coin_creator_vault_authority_bump: u8,
    ) -> Result {
        reset_fee_sharing_config::reset_fee_sharing_config(
            ctx,
            bonding_curve_bump,
            pump_creator_vault_bump,
            coin_creator_vault_authority_bump,
        )
    }

    /// Same confirmed mechanics as `reset_fee_sharing_config`, generalized to any
    /// quote mint: sweeps AMM-side coin-creator fees, distributes the vault's full
    /// pending balance to the *current* shareholder list, then overwrites
    /// `sharing_config` with a single 100%-share `new_admin` entry and bumps its
    /// version to 2.
    pub fn reset_fee_sharing_config_v2(
        ctx: Context<ResetFeeSharingConfigV2>,
        bonding_curve_bump: u8,
        pump_creator_vault_bump: u8,
        coin_creator_vault_authority_bump: u8,
    ) -> Result {
        reset_fee_sharing_config_v2::reset_fee_sharing_config_v2(
            ctx,
            bonding_curve_bump,
            pump_creator_vault_bump,
            coin_creator_vault_authority_bump,
        )
    }

    /// Update Fee Shares, make sure to distribute all the fees before calling this.
    /// Same confirmed mechanics as `update_fee_shares_v2`, WSOL-only.
    pub fn update_fee_shares(
        ctx: Context<UpdateFeeShares>,
        bonding_curve_bump: u8,
        pump_creator_vault_bump: u8,
        coin_creator_vault_authority_bump: u8,
        shareholders: ZcVec<Shareholder>,
    ) -> Result {
        update_fee_shares::update_fee_shares(
            ctx,
            bonding_curve_bump,
            pump_creator_vault_bump,
            coin_creator_vault_authority_bump,
            shareholders,
        )
    }

    /// Sets the final, one-shot shareholder list on `sharing_config`. Before applying
    /// it: (1) CPIs into `pump_amm` to sweep any AMM-side coin-creator fees into the
    /// bonding-curve vault, (2) CPIs into `pump::distribute_creator_fees_v2` to pay out
    /// any currently pending bonding-curve creator fees to the *current* (pre-update)
    /// shareholder list, then (3) overwrites `sharing_config.shareholders` and sets
    /// `admin_revoked = true` so no further call can succeed.
    pub fn update_fee_shares_v2(
        ctx: Context<UpdateFeeSharesV2>,
        bonding_curve_bump: u8,
        pump_creator_vault_bump: u8,
        coin_creator_vault_authority_bump: u8,
        shareholders: ZcVec<Shareholder>,
    ) -> Result {
        update_fee_shares_v2::update_fee_shares_v2(
            ctx,
            bonding_curve_bump,
            pump_creator_vault_bump,
            coin_creator_vault_authority_bump,
            shareholders,
        )
    }
}
