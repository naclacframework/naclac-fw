//! Token creator program entrypoint and instruction router.

#![no_std]

use {instructions::*, naclac_lang::prelude::*};

declare_id!("BXzs8zHVJBk7EwgBkpbTXsFq7RMZahhGduehdJd49hg4");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod systems;

#[program]
pub mod token_creator {
    /// Creates a classic Token mint of fixed supply `amount_token_pool +
    /// amount_token_launcher`, mints all of it to the payer, then seeds an AMM
    /// pool of `amount_token_pool` against `amount_quote` of the quote mint
    /// and locks the pool's first liquidity shares by reassigning their token
    /// account to the launch record, which no instruction ever moves. The mint
    /// authority is revoked, so the supply can never grow. Other wallets can
    /// still add liquidity. Fails if the payer already launched this `id`.
    pub fn launch_token(ctx: Context<LaunchToken>, args: LaunchTokenArgs) -> Result {
        launch_token::launch_token(ctx, args)
    }
}
