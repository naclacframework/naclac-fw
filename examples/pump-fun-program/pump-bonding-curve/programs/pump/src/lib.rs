#![no_std]
use naclac_lang::prelude::*;

declare_id!("rT4zemULLMgPUZq5fE6Gr6jTqsShFbwRbJcQSWG8gtY");

pub mod components;
pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;
pub mod systems;

use components::MinimumDistributableFeeEvent;
use events::{ClaimCashbackEvent, ClaimTokenIncentivesEvent, CollectCreatorFeeEvent};
use instructions::*;

#[program]
pub mod pump {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn create(
        ctx: Context<Create>,
        name: ZcString,
        symbol: ZcString,
        uri: ZcString,
        creator: Address,
        bonding_curve_bump: u8,
        metadata_bump: u8,
    ) -> Result {
        create::create(ctx, name, symbol, uri, creator, bonding_curve_bump, metadata_bump)
    }

    pub fn create_v2(
        ctx: Context<CreateV2>,
        name: ZcString,
        symbol: ZcString,
        uri: ZcString,
        creator: Address,
        is_cashback_enabled: Bool,
        bonding_curve_bump: u8,
    ) -> Result {
        create_v2::create_v2(ctx, name, symbol, uri, creator, is_cashback_enabled, bonding_curve_bump)
    }

    pub fn migrate_bonding_curve_creator(
        ctx: Context<MigrateBondingCurveCreator>,
        bonding_curve_bump: u8,
        sharing_config_bump: u8,
    ) -> Result {
        migrate_bonding_curve_creator::migrate_bonding_curve_creator(
            ctx,
            bonding_curve_bump,
            sharing_config_bump,
        )
    }

    pub fn set_params(ctx: Context<SetParams>, args: SetParamsArgs) -> Result {
        set_params::set_params(ctx, args)
    }

    pub fn update_buyback_config(
        ctx: Context<UpdateBuybackConfig>,
        buyback_basis_points: Option<u64>,
        buyback_vault_bumps: [u8; 8],
    ) -> Result {
        update_buyback_config::update_buyback_config(ctx, buyback_basis_points, buyback_vault_bumps)
    }

    pub fn update_global_authority(ctx: Context<UpdateGlobalAuthority>) -> Result {
        update_global_authority::update_global_authority(ctx)
    }

    pub fn admin_set_creator(
        ctx: Context<AdminSetCreator>,
        creator: Address,
        bonding_curve_bump: u8,
    ) -> Result {
        admin_set_creator::admin_set_creator(ctx, creator, bonding_curve_bump)
    }

    pub fn toggle_create_v2(ctx: Context<ToggleCreateV2>, enabled: Bool) -> Result {
        toggle_create_v2::toggle_create_v2(ctx, enabled)
    }

    pub fn toggle_mayhem_mode(ctx: Context<ToggleMayhemMode>, enabled: Bool) -> Result {
        toggle_mayhem_mode::toggle_mayhem_mode(ctx, enabled)
    }

    pub fn toggle_cashback_enabled(ctx: Context<ToggleCashbackEnabled>, enabled: Bool) -> Result {
        toggle_cashback_enabled::toggle_cashback_enabled(ctx, enabled)
    }

    pub fn set_reserved_fee_recipients(ctx: Context<SetReservedFeeRecipients>, whitelist_pda: Address) -> Result {
        set_reserved_fee_recipients::set_reserved_fee_recipients(ctx, whitelist_pda)
    }

    pub fn set_creator(
        ctx: Context<SetCreator>,
        creator: Address,
        metadata_bump: u8,
        bonding_curve_bump: u8,
    ) -> Result {
        set_creator::set_creator(ctx, creator, metadata_bump, bonding_curve_bump)
    }

    pub fn set_metaplex_creator(
        ctx: Context<SetMetaplexCreator>,
        metadata_bump: u8,
        bonding_curve_bump: u8,
    ) -> Result {
        set_metaplex_creator::set_metaplex_creator(ctx, metadata_bump, bonding_curve_bump)
    }

    pub fn distribute_creator_fees(
        ctx: Context<DistributeCreatorFees>,
        bonding_curve_bump: u8,
        creator_vault_bump: u8,
    ) -> Result {
        distribute_creator_fees::distribute_creator_fees(ctx, bonding_curve_bump, creator_vault_bump)
    }

    pub fn distribute_creator_fees_v2(
        ctx: Context<DistributeCreatorFeesV2>,
        bonding_curve_bump: u8,
        creator_vault_bump: u8,
        initialize_ata: Bool,
    ) -> Result {
        distribute_creator_fees_v2::distribute_creator_fees_v2(
            ctx,
            bonding_curve_bump,
            creator_vault_bump,
            initialize_ata,
        )
    }

    pub fn collect_creator_fee(
        ctx: Context<CollectCreatorFee>,
        creator_vault_bump: u8,
    ) -> Result<Option<CollectCreatorFeeEvent>> {
        collect_creator_fee::collect_creator_fee(ctx, creator_vault_bump)
    }

    pub fn collect_creator_fee_v2(
        ctx: Context<CollectCreatorFeeV2>,
        args: CollectCreatorFeeV2Args,
    ) -> Result<Option<CollectCreatorFeeEvent>> {
        collect_creator_fee_v2::collect_creator_fee_v2(ctx, args)
    }

    pub fn get_minimum_distributable_fee(
        ctx: Context<GetMinimumDistributableFee>,
        bonding_curve_bump: u8,
        creator_vault_bump: u8,
    ) -> Result<MinimumDistributableFeeEvent> {
        get_minimum_distributable_fee::get_minimum_distributable_fee(ctx, bonding_curve_bump, creator_vault_bump)
    }

    pub fn buy(ctx: Context<Buy>, args: BuyArgs) -> Result {
        buy::buy(ctx, args)
    }

    pub fn buy_exact_sol_in(ctx: Context<BuyExactSolIn>, args: BuyExactSolInArgs) -> Result {
        buy_exact_sol_in::buy_exact_sol_in(ctx, args)
    }

    pub fn buy_v2(ctx: Context<BuyV2>, args: BuyV2Args) -> Result {
        buy_v2::buy_v2(ctx, args)
    }

    pub fn buy_exact_quote_in_v2(ctx: Context<BuyExactQuoteInV2>, args: BuyExactQuoteInV2Args) -> Result {
        buy_exact_quote_in_v2::buy_exact_quote_in_v2(ctx, args)
    }

    pub fn sell(ctx: Context<Sell>, args: SellArgs) -> Result {
        sell::sell(ctx, args)
    }

    pub fn sell_v2(ctx: Context<SellV2>, args: SellV2Args) -> Result {
        sell_v2::sell_v2(ctx, args)
    }

    pub fn migrate(ctx: Context<Migrate>, args: MigrateArgs) -> Result {
        migrate::migrate(ctx, args)
    }

    pub fn migrate_v2(ctx: Context<MigrateV2>, args: MigrateV2Args) -> Result {
        migrate_v2::migrate_v2(ctx, args)
    }

    pub fn extend_account(ctx: Context<ExtendAccount>) -> Result {
        extend_account::extend_account(ctx)
    }

    pub fn add_quote_mint(ctx: Context<AddQuoteMint>, quote_mint: Address) -> Result {
        add_quote_mint::add_quote_mint(ctx, quote_mint)
    }

    pub fn remove_quote_mint(ctx: Context<RemoveQuoteMint>, quote_mint: Address) -> Result {
        remove_quote_mint::remove_quote_mint(ctx, quote_mint)
    }

    pub fn set_virtual_quote_reserves(
        ctx: Context<SetVirtualQuoteReserves>,
        initial_virtual_quote_reserves: u64,
    ) -> Result {
        set_virtual_quote_reserves::set_virtual_quote_reserves(ctx, initial_virtual_quote_reserves)
    }

    pub fn init_user_volume_accumulator(
        ctx: Context<InitUserVolumeAccumulator>,
        user_volume_accumulator_bump: u8,
    ) -> Result {
        init_user_volume_accumulator::init_user_volume_accumulator(ctx, user_volume_accumulator_bump)
    }

    pub fn close_user_volume_accumulator(ctx: Context<CloseUserVolumeAccumulator>) -> Result {
        close_user_volume_accumulator::close_user_volume_accumulator(ctx)
    }

    pub fn claim_cashback(ctx: Context<ClaimCashback>) -> Result<Option<ClaimCashbackEvent>> {
        claim_cashback::claim_cashback(ctx)
    }

    pub fn claim_cashback_v2(ctx: Context<ClaimCashbackV2>, args: ClaimCashbackV2Args) -> Result<Option<ClaimCashbackEvent>> {
        claim_cashback_v2::claim_cashback_v2(ctx, args)
    }

    pub fn claim_token_incentives(ctx: Context<ClaimTokenIncentives>, args: ClaimTokenIncentivesArgs) -> Result<Option<ClaimTokenIncentivesEvent>> {
        claim_token_incentives::claim_token_incentives(ctx, args)
    }
}
