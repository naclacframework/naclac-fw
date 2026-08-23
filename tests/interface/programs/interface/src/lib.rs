#![no_std]
use naclac_lang::prelude::*;

declare_id!("GyyYtT9woGRh7K9VAV8sZMkaB8Wq64uVdRjiuNXQTQhN");

pub mod instructions;

use instructions::*;

#[program]
pub mod interface_prog {
    pub fn check_token_interface(ctx: Context<CheckTokenInterface>) -> Result {
        check_token_interface::check_token_interface(ctx)
    }
}
