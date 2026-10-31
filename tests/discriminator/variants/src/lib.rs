#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod constants;
pub mod instructions;
pub mod state;

use instructions::*;

#[program]
pub mod discriminator {
    pub fn init_vault(ctx: Context<InitVault>) -> Result {
        init_vault::init_vault(ctx)
    }

    pub fn init_config(ctx: Context<InitConfig>) -> Result {
        init_config::init_config(ctx)
    }

    pub fn read_vault(ctx: Context<ReadVault>) -> Result {
        read_vault::read_vault(ctx)
    }

    pub fn touch_config(ctx: Context<TouchConfig>) -> Result {
        touch_config::touch_config(ctx)
    }
}
