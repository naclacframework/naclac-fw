#![no_std]
use naclac_lang::prelude::*;

declare_id!("3buQWRXdp8UE62AN4jnTKyb7xUDQ62GkMpcRgMgxpEgA");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod cpi_caller {
    pub fn init_caller_authority(ctx: Context<InitCallerAuthority>) -> Result {
        init_caller_authority::init_caller_authority(ctx)
    }

    pub fn call_system_transfer(ctx: Context<CallSystemTransfer>, amount: u64) -> Result {
        call_system_transfer::call_system_transfer(ctx, amount)
    }

    pub fn call_setup_counter(ctx: Context<CallSetupCounter>) -> Result {
        call_setup_counter::call_setup_counter(ctx)
    }

    pub fn call_authorized_increment(ctx: Context<CallAuthorizedIncrement>) -> Result {
        call_authorized_increment::call_authorized_increment(ctx)
    }
}
