#![no_std]
use naclac_lang::prelude::*;

declare_id!("EvHaZmAqALtVA6SQLcN1MZVrMRTY37odGCaLBwxBnhfy");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod dup_mut {
    pub fn init_vault_a(ctx: Context<InitVaultA>) -> Result {
        init_vault_a::init_vault_a(ctx)
    }

    pub fn init_vault_b(ctx: Context<InitVaultB>) -> Result {
        init_vault_b::init_vault_b(ctx)
    }

    pub fn touch_pair_no_alias(ctx: Context<TouchPairNoAlias>) -> Result {
        touch_pair_no_alias::touch_pair_no_alias(ctx)
    }

    pub fn touch_pair_with_alias(ctx: Context<TouchPairWithAlias>) -> Result {
        touch_pair_with_alias::touch_pair_with_alias(ctx)
    }
}
