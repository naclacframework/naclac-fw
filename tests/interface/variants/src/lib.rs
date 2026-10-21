#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod instructions;

use instructions::*;

#[program]
pub mod interface_prog {
    pub fn check_token_interface(ctx: Context<CheckTokenInterface>) -> Result {
        check_token_interface::check_token_interface(ctx)
    }
}
