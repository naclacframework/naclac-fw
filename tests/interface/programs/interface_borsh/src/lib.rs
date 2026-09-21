use naclac_lang::prelude::*;

declare_id!("XZkwemfFMCTKXmKw8qab7wZMr9hf6RcauBJw5pADFy7");

pub mod instructions;

use instructions::*;

#[program]
pub mod interface_borsh_prog {
    pub fn check_token_interface(ctx: Context<CheckTokenInterface>) -> Result {
        check_token_interface::check_token_interface(ctx)
    }
}
