#![no_std]
use naclac_lang::prelude::*;

declare_id!("rT4zemULLMgPUZq5fE6Gr6jTqsShFbwRbJcQSWG8gtY");

pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;
pub mod state;
pub mod systems;

use events::{ClaimCashbackEvent, ClaimTokenIncentivesEvent, CollectCreatorFeeEvent};
use instructions::*;
use state::MinimumDistributableFeeEvent;

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

    /// Sweeps `creator_vault` down to its rent-exempt minimum, paying the swept
    /// amount pro-rata by `share_bps` to `remaining_accounts` — one entry per
    /// `sharing_config.shareholders`, in order, bare pubkeys (native SOL only;
    /// `_v2` handles non-native quotes). Rounding: every shareholder but the last
    /// gets `floor(available * share_bps / 10_000)`; the last gets the exact
    /// remainder, so the vault never retains dust above its rent-exempt floor.
    /// This rounding rule is a reasonable implementation choice, not verified
    /// against the real bytecode's exact behavior for >1 shareholder.
    ///
    /// Requires `bonding_curve.creator == sharing_config`'s own address (real,
    /// live-confirmed check, `reference/fee-tier-probe/src/bin/probe71.rs`) --
    /// without it, `creator_vault` (derived from `bonding_curve.creator`) and the
    /// payout list (`sharing_config.shareholders`, from whichever `sharing_config`
    /// the caller passes) would have nothing tying them together, letting anyone
    /// redirect any bonding curve's real accumulated fees to an unrelated
    /// `sharing_config` they control. Also rejects any executable shareholder
    /// recipient (real, live-confirmed check, `reference/fee-tier-probe/src/bin/probe73.rs`)
    /// — an executable account can't receive lamports, so a stale shareholder
    /// entry that's since become a program account must be removed via
    /// `update_fee_shares(_v2)` first rather than silently failing the whole
    /// distribution at the transfer step.
    pub fn distribute_creator_fees(
        ctx: Context<DistributeCreatorFees>,
        bonding_curve_bump: u8,
        creator_vault_bump: u8,
    ) -> Result {
        distribute_creator_fees::distribute_creator_fees(ctx, bonding_curve_bump, creator_vault_bump)
    }

    /// Real `distribute_creator_fees_v2` supports arbitrary `quote_mint`s; this
    /// scoped pass only implements the wrapped-SOL path (matches
    /// `bonding-curve-01-overview.md`'s SOL-quote-only scope decision), and
    /// reverts `UnsupportedQuoteMint` for anything else. The WSOL path's
    /// distribution math and rounding rule are identical to `distribute_creator_fees`
    /// (v1) — see that instruction's doc comment.
    ///
    /// Requires `bonding_curve.creator == sharing_config`'s own address (real,
    /// live-confirmed check, `reference/fee-tier-probe/src/bin/probe71.rs`) --
    /// see `distribute_creator_fees`'s own doc comment for why. Also rejects any
    /// executable shareholder recipient (real, live-confirmed check,
    /// `reference/fee-tier-probe/src/bin/probe73.rs`) — see that same doc
    /// comment for why.
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

    /// Sweeps `creator_vault` down to its rent-exempt minimum, paying the full
    /// swept amount to `creator` — the native-SOL sibling of
    /// `distribute_creator_fees`'s own rent-exempt-floor sweep, just without a
    /// shareholder split (100% to `creator`). Permissionless: anyone may crank
    /// it, funds always land at the fixed `creator_vault` -> `creator` PDA pair.
    pub fn collect_creator_fee(
        ctx: Context<CollectCreatorFee>,
        creator_vault_bump: u8,
    ) -> Result<Option<CollectCreatorFeeEvent>> {
        collect_creator_fee::collect_creator_fee(ctx, creator_vault_bump)
    }

    /// Sweeps `creator_vault_token_account`'s full SPL-token balance to
    /// `creator_token_account` — the SPL-token sibling of `collect_creator_fee`
    /// (native SOL). Permissionless: anyone may crank it, funds always land at
    /// the fixed `creator_vault_token_account` -> `creator_token_account` pair.
    /// Unlike the native path there's no rent-exempt floor to preserve (an SPL
    /// token account's rent is a fixed lamport reserve separate from its token
    /// balance), so the entire token balance is swept.
    pub fn collect_creator_fee_v2(
        ctx: Context<CollectCreatorFeeV2>,
        args: CollectCreatorFeeV2Args,
    ) -> Result<Option<CollectCreatorFeeEvent>> {
        collect_creator_fee_v2::collect_creator_fee_v2(ctx, args)
    }

    /// Permissionless view instruction (confirmed via real IDL doc comment) that
    /// reports whether `creator_vault` currently holds enough to be worth
    /// cranking via `distribute_creator_fees(_v2)`. Real formula, empirically
    /// confirmed against real deployed `pump.so` (`reference/fee-tier-probe/src/bin/probe71.rs`,
    /// not inferred): `minimum_required` is a constant `2 *
    /// CREATOR_VAULT_RENT_EXEMPT_MINIMUM`; `distributable_fees` is
    /// `creator_vault`'s raw lamport balance down to the single rent-exempt
    /// floor (matching `distribute_creator_fees`'s own calculation exactly);
    /// `can_distribute` compares the *raw* `creator_vault` balance (not
    /// `distributable_fees`) against `minimum_required`, inclusive (`>=`). Like
    /// `pump_fees::get_fees`, this is a plain returned value, never `emit!`'d,
    /// despite the real IDL naming the return type `*Event`.
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

    /// Grows a pump-owned account's data buffer to match its currently
    /// compiled-in size, funding the extra rent from `user`. `account` stays a
    /// raw `AccountInfo` (never a typed `Account<T>`) deliberately: a
    /// still-undersized account -- the very thing being extended -- fails
    /// `Account<T>`'s own size check before this instruction's `realloc`
    /// constraint (let alone its body) ever runs (confirmed empirically:
    /// `AccountDataTooSmall` from a typed field against this exact scenario).
    /// Recognizes every account this program itself owns; add a new type to
    /// the `realloc::any_of` list above when a new account needs the same
    /// treatment.
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

    /// Creates a brand-new, zeroed `UserVolumeAccumulator` PDA for `user`,
    /// funded by `payer` -- permissionless, matching the real program's own
    /// account list (no signer requirement on `user` at all).
    pub fn init_user_volume_accumulator(
        ctx: Context<InitUserVolumeAccumulator>,
        user_volume_accumulator_bump: u8,
    ) -> Result {
        init_user_volume_accumulator::init_user_volume_accumulator(ctx, user_volume_accumulator_bump)
    }

    /// Closes `user`'s own `UserVolumeAccumulator`, reclaiming its rent to
    /// `user`. Unconditional -- no precondition on any pending reward field,
    /// confirmed against the real deployed program.
    pub fn close_user_volume_accumulator(ctx: Context<CloseUserVolumeAccumulator>) -> Result {
        close_user_volume_accumulator::close_user_volume_accumulator(ctx)
    }

    /// Sweeps `user_volume_accumulator`'s native-SOL cashback balance down to
    /// its rent-exempt minimum, paying the swept amount to `user` -- confirmed
    /// mechanism (`fees-07-donation-relay-progress.md`'s `probe16`): cashback
    /// lives as real native lamports directly in `user_volume_accumulator`'s
    /// own account balance (topped up by `buy`/`sell` on cashback-enabled
    /// coins); `cashback_earned` is only a running-total tracker, not itself
    /// spendable. `user_volume_accumulator` is owned by this program, so the
    /// debit is a direct `sub_lamports` (no signed CPI needed, unlike
    /// `creator_vault`'s System-owned sweep in `distribute_creator_fees`).
    /// Permissionless, matching the real account list (no signer requirement
    /// on `user`).
    pub fn claim_cashback(ctx: Context<ClaimCashback>) -> Result<Option<ClaimCashbackEvent>> {
        claim_cashback::claim_cashback(ctx)
    }

    /// Sweeps `associated_user_volume_accumulator`'s full SPL-token "stable
    /// cashback" balance to `associated_quote_user` -- the SPL-token sibling of
    /// `claim_cashback` (native SOL). No rent-exempt floor to preserve (an SPL
    /// token account's rent is a fixed lamport reserve separate from its token
    /// balance), so the entire balance is swept, matching
    /// `collect_creator_fee_v2`'s identical shape. Permissionless (no signer
    /// requirement on `user`).
    pub fn claim_cashback_v2(ctx: Context<ClaimCashbackV2>, args: ClaimCashbackV2Args) -> Result<Option<ClaimCashbackEvent>> {
        claim_cashback_v2::claim_cashback_v2(ctx, args)
    }

    /// Sweeps `user_volume_accumulator`'s full `total_unclaimed_tokens` balance
    /// out of the shared `global_incentive_token_account` pool into `user_ata`,
    /// signed by `global_volume_accumulator`'s own literal-seed PDA authority
    /// (same `bare bump` + `ctx.bumps.*` idiom already established for
    /// `pump_fees_authority` elsewhere in this program). Mirrors
    /// `collect_creator_fee_v2`'s full-balance-sweep shape (no partial claims).
    pub fn claim_token_incentives(ctx: Context<ClaimTokenIncentives>, args: ClaimTokenIncentivesArgs) -> Result<Option<ClaimTokenIncentivesEvent>> {
        claim_token_incentives::claim_token_incentives(ctx, args)
    }
}
