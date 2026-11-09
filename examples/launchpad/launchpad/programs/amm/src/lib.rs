//! Automated market maker program entrypoint and instruction router.

#![no_std]

#[cfg(test)]
extern crate std;

use {
    instructions::*,
    naclac_lang::prelude::*,
};

declare_id!("3BpKpLxLuskuhh1xjK51dmTPpu5CmhekKTNg3RWBx3qo");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod systems;
pub mod validation;

#[program]
pub mod amm {
    /// Creates an AMM pool for two mints in ascending address order and seeds it
    /// with `amount_a` and `amount_b` from the payer's token accounts, minting the
    /// first LP shares to the payer. A mint's token program is the one that owns
    /// it, and a Token-2022 mint is accepted only with informational extensions.
    pub fn initialize(ctx: Context<Initialize>, id: u64, amount_a: u64, amount_b: u64) -> Result {
        initialize::initialize(ctx, id, amount_a, amount_b)
    }

    /// Swaps an exact input amount of one pool token for output tokens of the other.
    /// The input leg moves through the token program of the input mint and the
    /// output leg through the token program of the output mint.
    pub fn swap(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result {
        swap::swap(ctx, amount_in, minimum_amount_out)
    }

    /// Adds liquidity to the AMM pool proportionally and mints LP tokens to the user.
    /// Takes at most `max_amount_a` and `max_amount_b`, and fails unless at least
    /// `min_lp` shares are minted.
    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        max_amount_a: u64,
        max_amount_b: u64,
        min_lp: u64,
    ) -> Result {
        add_liquidity::add_liquidity(ctx, max_amount_a, max_amount_b, min_lp)
    }

    /// Removes liquidity by burning user's LP tokens and returning proportional pool assets.
    /// Fails unless at least `min_amount_a` and `min_amount_b` are returned.
    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_amount: u64,
        min_amount_a: u64,
        min_amount_b: u64,
    ) -> Result {
        remove_liquidity::remove_liquidity(ctx, lp_amount, min_amount_a, min_amount_b)
    }
}
