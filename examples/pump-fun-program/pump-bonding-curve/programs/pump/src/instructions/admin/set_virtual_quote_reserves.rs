use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(initial_virtual_quote_reserves: u64)]
pub struct SetVirtualQuoteReserves {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

#[instruction]
pub fn set_virtual_quote_reserves(
    ctx: Context<SetVirtualQuoteReserves>,
    initial_virtual_quote_reserves: u64,
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    ctx.accounts.global.initial_virtual_quote_reserves = initial_virtual_quote_reserves;

    msg!("Virtual quote reserves successfully updated");
    Ok(())
}
