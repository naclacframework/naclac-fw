use naclac_lang::prelude::*;

declare_id!("ELAcXuQJ9UHTCcrrSHfnMRxzyxy8yh51oMu5BVJPCy8h");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod amm {
   pub fn initialize(ctx: Context<Initialize>, id: u64, pool_bump: u8, amount_a: u64, amount_b: u64) -> Result {
        initialize::initialize(ctx, id, pool_bump, amount_a, amount_b)
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result {
        swap::swap(ctx, amount_in, minimum_amount_out)
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, max_amount_a: u64, max_amount_b: u64) -> Result {
        add_liquidity::add_liquidity(ctx, max_amount_a, max_amount_b)
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64) -> Result {
        remove_liquidity::remove_liquidity(ctx, lp_amount)
    }
}
