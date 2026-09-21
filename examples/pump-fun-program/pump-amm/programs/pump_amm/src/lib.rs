#![no_std]
use naclac_lang::prelude::*;

declare_id!("CgRf7F42guD313ikFQJQaodeqayL3XH8R2oy8KfAStfD");

pub mod components;
pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;
pub mod systems;

use instructions::*;

#[program]
pub mod pump_amm {
    pub fn create_config(ctx: Context<CreateConfig>) -> Result {
        create_config::create_config(ctx)
    }

    pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result {
        create_pool::create_pool(ctx, args)
    }

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

    pub fn init_boost(
        ctx: Context<InitBoost>,
        boost_vault_authority_bump: u8,
    ) -> Result {
        init_boost::init_boost(ctx, boost_vault_authority_bump)
    }

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
