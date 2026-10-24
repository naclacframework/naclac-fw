#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod pda_seeds {
    pub fn init_registry(ctx: Context<InitRegistry>) -> Result {
        init_registry::init_registry(ctx)
    }

    pub fn init_entry(ctx: Context<InitEntry>, bump: u8) -> Result {
        init_entry::init_entry(ctx, bump)
    }

    pub fn touch_entry_bare_bump(ctx: Context<TouchEntryBareBump>) -> Result {
        touch_entry_bare_bump::touch_entry_bare_bump(ctx)
    }

    pub fn touch_registry_explicit_bump(
        ctx: Context<TouchRegistryExplicitBump>,
        bump: u8,
    ) -> Result {
        touch_registry_explicit_bump::touch_registry_explicit_bump(ctx, bump)
    }

    pub fn init_child(ctx: Context<InitChild>, bump: u8) -> Result {
        init_child::init_child(ctx, bump)
    }

    pub fn init_child_safe(ctx: Context<InitChildSafe>) -> Result {
        init_child_safe::init_child_safe(ctx)
    }

    pub fn init_tagged_child(ctx: Context<InitTaggedChild>, bump: u8) -> Result {
        init_tagged_child::init_tagged_child(ctx, bump)
    }

    pub fn init_child_late(ctx: Context<InitChildLate>) -> Result {
        init_child_late::init_child_late(ctx)
    }

    pub fn init_entry_late(ctx: Context<InitEntryLate>) -> Result {
        init_entry_late::init_entry_late(ctx)
    }

    pub fn touch_config_entry_bare_bump(ctx: Context<TouchConfigEntryBareBump>) -> Result {
        touch_config_entry_bare_bump::touch_config_entry_bare_bump(ctx)
    }

    pub fn touch_config_entry_bare_bump_with_args(
        ctx: Context<TouchConfigEntryBareBumpWithArgs>,
        is_pump_pool: Bool,
        market_cap_lamports: u128,
        trade_size_lamports: u64,
        is_new_quote_mint: Bool,
    ) -> Result {
        touch_config_entry_bare_bump_with_args::touch_config_entry_bare_bump_with_args(
            ctx,
            is_pump_pool,
            market_cap_lamports,
            trade_size_lamports,
            is_new_quote_mint,
        )
    }

    pub fn read_config_entry_bare_bump(
        ctx: Context<ReadConfigEntryBareBump>,
        is_pump_pool: Bool,
        market_cap_lamports: u128,
        trade_size_lamports: u64,
        is_new_quote_mint: Bool,
    ) -> Result<u64> {
        read_config_entry_bare_bump::read_config_entry_bare_bump(
            ctx,
            is_pump_pool,
            market_cap_lamports,
            trade_size_lamports,
            is_new_quote_mint,
        )
    }
}
