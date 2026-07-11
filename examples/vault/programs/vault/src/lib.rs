#![no_std]
use naclac_lang::prelude::*;

declare_id!("2ZTMxbh1yJFNJsH3eM3dh5XfBQVrEEExQqXQsMoJBgt2");

pub mod components;
pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod systems;

use instructions::*;

#[program]
pub mod vault {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64, user_bump: u8) -> Result {
        deposit::deposit(ctx, amount, user_bump)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result {
        withdraw::withdraw(ctx, amount)
    }
}
