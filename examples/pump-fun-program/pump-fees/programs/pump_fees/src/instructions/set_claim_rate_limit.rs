use naclac_lang::prelude::*;
use crate::components::FeeProgramGlobal;
use crate::constants::FEE_PROGRAM_GLOBAL_SEED;
use crate::errors::FeesError;
use crate::events::SetClaimRateLimitEvent;

#[derive(Accounts)]
pub struct SetClaimRateLimit {
    pub authority: Signer,
    #[account(mut, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
}

pub fn set_claim_rate_limit(ctx: Context<SetClaimRateLimit>, claim_rate_limit: u64) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.fee_program_global.authority,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;
    ctx.accounts.fee_program_global.claim_rate_limit = claim_rate_limit;

    emit!(SetClaimRateLimitEvent {
        timestamp,
        claim_rate_limit,
    });

    Ok(())
}
