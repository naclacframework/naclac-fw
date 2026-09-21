use naclac_lang::prelude::*;
use crate::components::{FeeProgramGlobal, Global};
use crate::constants::{FEE_PROGRAM_GLOBAL_SEED, PUMP_GLOBAL_SEED, PUMP_PROGRAM_ID};
use crate::errors::FeesError;
use crate::events::InitializeFeeProgramGlobalEvent;

#[derive(Accounts)]
pub struct InitializeFeeProgramGlobal {
    #[account(mut)]
    pub authority: Signer,
    #[account(seeds = [PUMP_GLOBAL_SEED], seeds::program = PUMP_PROGRAM_ID, owner = PUMP_PROGRAM_ID)]
    pub pump_global: Account<Global>,
    #[account(init, payer = authority, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
    pub system_program: Program<System>,
}

pub fn initialize_fee_program_global(
    ctx: Context<InitializeFeeProgramGlobal>,
    social_claim_authority: Address,
    disable_flags: u8,
    claim_rate_limit: u64,
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.pump_global.authority,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;

    let fee_program_global = &mut ctx.accounts.fee_program_global;
    fee_program_global.authority = ctx.accounts.authority.address();
    fee_program_global.social_claim_authority = social_claim_authority;
    fee_program_global.disable_flags = disable_flags;
    fee_program_global.claim_rate_limit = claim_rate_limit;

    emit!(InitializeFeeProgramGlobalEvent {
        timestamp,
        claim_rate_limit,
        authority: ctx.accounts.authority.address(),
        social_claim_authority,
        disable_flags,
    });

    Ok(())
}
