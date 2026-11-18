//! Public L1 prediction pools; private execution is integrated separately.

#![no_std]

use {instructions::*, naclac_lang::prelude::*};

declare_id!("9tm2sLjXouT9SKndmGc4RYp78DtSPq5HjQyCH37oT5Rx");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

#[program]
pub mod swiv_privacy {
    use super::*;

    pub fn initialize_protocol(ctx: Context<InitializeProtocol>, protocol_fee_bps: u64) -> Result {
        admin::initialize_protocol(ctx, protocol_fee_bps)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_treasury: Opt<Address>,
        new_protocol_fee_bps: Opt<u64>,
        new_batch_settle_wait_duration: Opt<i64>,
    ) -> Result {
        admin::update_config(
            ctx,
            new_treasury,
            new_protocol_fee_bps,
            new_batch_settle_wait_duration,
        )
    }

    pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Address) -> Result {
        admin::transfer_admin(ctx, new_admin)
    }

    pub fn set_pause(ctx: Context<SetPause>, paused: bool) -> Result {
        admin::set_pause(ctx, paused)
    }

    pub fn create_pool(
        ctx: Context<CreatePool>,
        title: FixedString<128>,
        start_time: i64,
        end_time: i64,
        max_accuracy_buffer: u64,
        conviction_bonus_bps: u64,
    ) -> Result {
        pool::create_pool(
            ctx,
            title,
            start_time,
            end_time,
            max_accuracy_buffer,
            conviction_bonus_bps,
        )
    }

    pub fn init_bet(ctx: Context<InitBet>, amount: u64) -> Result {
        betting::init_bet(ctx, amount)
    }

    pub fn place_bet(ctx: Context<PlaceBet>, prediction: u64) -> Result {
        betting::place_bet(ctx, prediction)
    }

    pub fn update_bet(
        ctx: Context<UpdateBet>,
        new_prediction: u64,
        additional_stake: u64,
    ) -> Result {
        betting::update_bet(ctx, new_prediction, additional_stake)
    }

    pub fn add_stake(ctx: Context<UpdateBet>, amount: u64) -> Result {
        betting::add_stake(ctx, amount)
    }

    pub fn resolve_pool(ctx: Context<ManagePool>, final_outcome: u64) -> Result {
        pool::resolve_pool(ctx, final_outcome)
    }

    pub fn cancel_pool(ctx: Context<ManagePool>) -> Result {
        pool::cancel_pool(ctx)
    }

    pub fn batch_calculate_weights(ctx: Context<BatchCalculateWeights>) -> Result {
        pool::batch_calculate_weights(ctx)
    }

    pub fn finalize_weights(ctx: Context<FinalizeWeights>) -> Result {
        pool::finalize_weights(ctx)
    }

    pub fn claim_reward(ctx: Context<Withdraw>) -> Result {
        payouts::claim_reward(ctx)
    }

    pub fn emergency_refund(ctx: Context<Withdraw>) -> Result {
        payouts::emergency_refund(ctx)
    }

    pub fn close_pool(ctx: Context<ClosePool>) -> Result {
        payouts::close_pool(ctx)
    }
}
