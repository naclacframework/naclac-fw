#![no_std]
use naclac_lang::prelude::*;

declare_id!("A35nKmg8KP7TNmwxdhBae9v21WFkpNvrCjbNnrqDSJBg");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod cpi_callee {
    pub fn init_counter(ctx: Context<InitCounter>) -> Result {
        init_counter::init_counter(ctx)
    }

    pub fn authorized_increment(ctx: Context<AuthorizedIncrement>) -> Result {
        authorized_increment::authorized_increment(ctx)
    }
}
