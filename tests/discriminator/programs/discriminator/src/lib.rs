#![no_std]
use naclac_lang::prelude::*;

declare_id!("DvcTxn5iTLsnGYLZJeVS5Fu3zVZLiJbmTFPqGDduUc9j");

pub mod components;
pub mod constants;
pub mod instructions;

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
}
