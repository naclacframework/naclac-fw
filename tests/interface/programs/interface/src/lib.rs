#![no_std]
use naclac_lang::prelude::*;

declare_id!("3TYYaU6cA43rok7UVrRQ6DwaUaag7KK72qogto6gqj4R");

pub mod instructions;

use instructions::*;

#[program]
pub mod interface_prog {
    pub fn check_token_interface(ctx: Context<CheckTokenInterface>) -> Result {
        check_token_interface::check_token_interface(ctx)
    }
}
