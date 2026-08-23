use naclac_lang::prelude::*;

declare_id!("AfERbyQZJP6FteX4WpwCKrRUqiNSNN7T3ak36ePoX2y");

pub mod instructions;

use instructions::*;

#[program]
pub mod interface_borsh_prog {
    pub fn check_token_interface(ctx: Context<CheckTokenInterface>) -> Result {
        check_token_interface::check_token_interface(ctx)
    }
}
