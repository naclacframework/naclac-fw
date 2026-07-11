use naclac_lang::prelude::*;

declare_id!("GuuwA59rpuqEaQW5zZzdkteq1Tsrm1qWfnwZQHx9abwD");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod token_vault_zc {
    pub fn initialize(ctx: Context<Initialize>, vault_id: u64, vault_bump: u8) -> Result {
        initialize::initialize(ctx, vault_id, vault_bump)
    }

    pub fn deposit(ctx: Context<Deposit>, vault_id: u64, amount: u64, user_bump: u8) -> Result {
        deposit::deposit(ctx, vault_id, amount, user_bump)
    }

    pub fn withdraw(ctx: Context<Withdraw>, vault_id: u64, amount: u64) -> Result {
        withdraw::withdraw(ctx, vault_id, amount)
    }
}
