use naclac_lang::prelude::*;

declare_id!("9j8UexsZ6UFq7B8as4vXPLtzVN4XyBsZzqhfvmGpNfCB");

pub mod components;
pub mod instructions;
pub mod events;
pub mod errors;
pub mod constants;
pub mod systems;

use instructions::*;

#[program]
pub mod token_creator {
    pub fn launch_token(
        ctx: Context<LaunchToken>,
        args: LaunchTokenArgs,
    ) -> Result {
        launch_token::launch_token(ctx, args)
    }

    pub fn create_mint(
        ctx: Context<CreateMint>,
        id: u64,
        mint_bump: u8,
        launch_record_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint::create_mint(
            ctx,
            id,
            mint_bump,
            launch_record_bump,
            decimals,
        )
    }
}
