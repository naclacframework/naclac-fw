#![cfg_attr(not(feature = "idl-build"), no_std)]
use naclac_lang::prelude::*;

declare_id!("C4gz1yWQvjVbFA3nxGRTGGzyMHaz6uMHqyemC78LqZJT");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod counter {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
