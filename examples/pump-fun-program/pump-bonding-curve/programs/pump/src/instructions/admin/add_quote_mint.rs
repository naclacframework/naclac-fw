use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::{GLOBAL_SEED, WSOL_MINT};
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(quote_mint: Address)]
pub struct AddQuoteMint {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

#[instruction]
pub fn add_quote_mint(ctx: Context<AddQuoteMint>, quote_mint: Address) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );
    require!(
        quote_mint != Address::default() && quote_mint != WSOL_MINT,
        PumpError::QuoteMintNotEligibleForWhitelist
    );

    let mints = &mut ctx.accounts.global.whitelisted_quote_mints;
    require!(!mints.contains(&quote_mint), PumpError::QuoteMintAlreadyWhitelisted);

    let slot = mints.iter_mut().find(|m| **m == Address::default());
    match slot {
        Some(slot) => *slot = quote_mint,
        None => return Err(PumpError::QuoteMintWhitelistFull.into()),
    }

    msg!("Quote mint successfully added");
    Ok(())
}
