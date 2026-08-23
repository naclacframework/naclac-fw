use naclac_lang::prelude::*;
use crate::components::FeeProgramGlobal;
use crate::constants::FEE_PROGRAM_GLOBAL_SEED;
use crate::errors::FeesError;
use crate::events::SetSocialClaimAuthorityEvent;

#[derive(Accounts)]
pub struct SetSocialClaimAuthority {
    pub authority: Signer,
    #[account(mut, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
}

#[instruction]
pub fn set_social_claim_authority(
    ctx: Context<SetSocialClaimAuthority>,
    social_claim_authority: Address,
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.fee_program_global.authority,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;
    ctx.accounts.fee_program_global.social_claim_authority = social_claim_authority;

    emit!(SetSocialClaimAuthorityEvent {
        timestamp,
        social_claim_authority,
    });

    Ok(())
}
