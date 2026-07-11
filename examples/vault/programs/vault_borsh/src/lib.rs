use naclac_lang::prelude::*;

declare_id!("3Rd7M1sS1asY9YZTPpHQMz3NFw75XDvDhWYuT6pr2kNd");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod vault_borsh {
    
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
