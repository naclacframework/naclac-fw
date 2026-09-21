use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::{GLOBAL_SEED, WSOL_MINT};
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(quote_mint: Address)]
pub struct RemoveQuoteMint {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

pub fn remove_quote_mint(ctx: Context<RemoveQuoteMint>, quote_mint: Address) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );
    require!(
        quote_mint != Address::default() && quote_mint != WSOL_MINT,
        PumpError::QuoteMintNotEligibleForWhitelist
    );

    let mints = &mut ctx.accounts.global.whitelisted_quote_mints;
    let slot = mints.iter_mut().find(|m| **m == quote_mint);
    match slot {
        Some(slot) => *slot = Address::default(),
        None => return Err(PumpError::QuoteMintNotWhitelisted.into()),
    }

    msg!("Quote mint successfully removed");
    Ok(())
}
