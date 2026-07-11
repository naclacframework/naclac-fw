#![no_std]
use naclac_lang::prelude::*;

declare_id!("CftDHdXFWzSiY7AbXsJcBWVhWGx1qpk2TYbRtEeDvrX1");

pub mod components;
pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod escrow {
    pub fn make(ctx: Context<Make>, seed: u64, escrow_bump: u8, amount_a: u64, amount_b: u64) -> Result {
        make::make(ctx, seed, escrow_bump, amount_a, amount_b)
    }

    pub fn take(ctx: Context<Take>, seed: u64) -> Result {
        take::take(ctx, seed)
    }

    pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {
        cancel::cancel(ctx, seed)
    }
}
