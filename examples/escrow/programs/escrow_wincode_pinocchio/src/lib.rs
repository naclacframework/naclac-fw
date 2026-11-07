#![no_std]
use naclac_lang::prelude::*;

declare_id!("GXHh7jtpFHtpvJRQWwgpU53diSw94MFoDTq2YeHmuDUu");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

#[program]
pub mod escrow_wincode_pinocchio {
    pub fn make(ctx: Context<Make>, id: u64, amount_a: u64, amount_b: u64) -> Result {
        make::make(ctx, id, amount_a, amount_b)
    }

    pub fn take(ctx: Context<Take>, id: u64) -> Result {
        take::take(ctx, id)
    }

    pub fn cancel(ctx: Context<Cancel>, id: u64) -> Result {
        cancel::cancel(ctx, id)
    }
}
