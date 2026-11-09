#![no_std]
use naclac_lang::prelude::*;

declare_id!("CgRf7F42guD313ikFQJQaodeqayL3XH8R2oy8KfAStfD");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod systems;

use instructions::*;

#[program]
pub mod pump_amm {
    /// Creates the singleton `GlobalConfig`, gated by the project's own
    /// `ADMIN_PUBKEY` (see its doc comment for why this diverges from real
    /// `pump_amm`'s hardcoded admin). Leaves `boost_enabled`/`boost_authority`
    /// at their zero-init defaults -- `toggle_boost`/`set_boost_authority`
    /// (both gated on `global_config.admin`, set here) enable them separately,
    /// matching real `create_config`'s own args, which don't include boost
    /// fields at all.
    pub fn create_config(ctx: Context<CreateConfig>) -> Result {
        create_config::create_config(ctx)
    }

    pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result {
        create_pool::create_pool(ctx, args)
    }

    /// Sweeps `coin_creator_vault_ata` (a wrapped-SOL account) into `pump_creator_vault`
    /// via the real program's documented close-recreate-forward sequence (see
    /// `docs/instructions/CREATOR_FEE_SHARING.md`, empirically confirmed end-to-end
    /// via `reference/fee-tier-probe/src/bin/probe11.rs`): close the ATA (forwarding
    /// its full lamports, wrapped amount + rent, to `coin_creator_vault_authority`),
    /// recreate a fresh zero-balance ATA at the same address (self-funded from those
    /// lamports), then forward the remainder on to `pump_creator_vault`. Real
    /// docstring: "If coin creator fees are currently below rent.minimum_balance
    /// (TokenAccount::LEN) the transfer will be skipped" — implemented below as an
    /// early no-op rather than an error.
    pub fn transfer_creator_fees_to_pump(
        ctx: Context<TransferCreatorFeesToPump>,
        coin_creator_vault_authority_bump: u8,
        pump_creator_vault_bump: u8,
    ) -> Result {
        transfer_creator_fees_to_pump::transfer_creator_fees_to_pump(
            ctx,
            coin_creator_vault_authority_bump,
            pump_creator_vault_bump,
        )
    }

    /// Real `transfer_creator_fees_to_pump_v2` supports arbitrary `quote_mint`s;
    /// this scoped pass only implements the wrapped-SOL path (matches
    /// `bonding-curve-01-overview.md`'s SOL-quote-only scope decision), and
    /// reverts `UnsupportedQuoteMint` for anything else. The WSOL path's
    /// mechanics are identical to `transfer_creator_fees_to_pump` (v1) — see
    /// that instruction's doc comment.
    pub fn transfer_creator_fees_to_pump_v2(
        ctx: Context<TransferCreatorFeesToPumpV2>,
        coin_creator_vault_authority_bump: u8,
        pump_creator_vault_bump: u8,
    ) -> Result {
        transfer_creator_fees_to_pump_v2::transfer_creator_fees_to_pump_v2(
            ctx,
            coin_creator_vault_authority_bump,
            pump_creator_vault_bump,
        )
    }

    /// Real behavior confirmed via `probe46.rs`/`probe48.rs`/`probe49.rs`
    /// (litesvm, real bytecode) and 5 independent real mainnet `migrate_v2`
    /// transactions (`docs/plan/amm-03-boost-mechanism.md`): `init_boost` moves
    /// no funds of its own accord and computes no percentage — it reads
    /// `pool_quote_token_account`'s and `pool_base_token_account`'s balances
    /// *as they stand at call time*, computes `virtual_quote_reserves =
    /// floor(quote_balance * base_balance / BOOST_BASE_SUPPLY_DIVISOR)`,
    /// transfers that amount out of `pool_quote_token_account` into a
    /// freshly-created `boost_vault`, and records it on `pool`. Confirmed with
    /// zero error across 5 independent real transactions plus a direct blind
    /// litesvm replication using a 9-decimal quote mint (ruling out a
    /// quote-decimals dependency).
    pub fn init_boost(
        ctx: Context<InitBoost>,
        boost_vault_authority_bump: u8,
    ) -> Result {
        init_boost::init_boost(ctx, boost_vault_authority_bump)
    }

    /// Real formula confirmed via `probe46.rs` (litesvm, real bytecode),
    /// `docs/plan/amm-03-boost-mechanism.md`: internally a genuine constant-
    /// product buy against the pool using `effective_quote_reserves = real +
    /// virtual` as the pricing denominator, with the same "amount − 1"
    /// conservative-rounding pattern already confirmed for `buy_exact_sol_in`
    /// in the bonding-curve program: `base_out = floor(base_reserves *
    /// (net_quote - 1) / (effective_quote_reserves + net_quote - 1))`. 100% of
    /// `base_out` is burned (no partial-burn leftover observed).
    /// `pool.virtual_quote_reserves` is left completely unchanged — it's a
    /// fixed pricing-depth constant set once by `init_boost`, not a running
    /// counter.
    ///
    /// `quote_amount_in_used` is capped at `boost_vault`'s current balance —
    /// this specific capping behavior (vs. e.g. erroring outright) was not
    /// empirically exercised (the one real/probe example available always had
    /// `quote_amount_in_used == quote_amount_in_requested`), inferred from the
    /// event's separate `_requested`/`_used` fields, which would otherwise have
    /// no reason to differ.
    pub fn boost_buy_and_burn(
        ctx: Context<BoostBuyAndBurn>,
        quote_amount_in: u64,
        min_base_amount_burned: u64,
        boost_vault_authority_bump: u8,
    ) -> Result {
        boost_buy_and_burn::boost_buy_and_burn(
            ctx,
            quote_amount_in,
            min_base_amount_burned,
            boost_vault_authority_bump,
        )
    }

    pub fn toggle_boost(ctx: Context<ToggleBoost>, enabled: Bool) -> Result {
        toggle_boost::toggle_boost(ctx, enabled)
    }

    pub fn set_boost_authority(ctx: Context<SetBoostAuthority>) -> Result {
        set_boost_authority::set_boost_authority(ctx)
    }
}
