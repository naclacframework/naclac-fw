use naclac_lang::prelude::*;
use crate::components::{FeeProgramGlobal, SocialFeePda};
use crate::constants::SOCIAL_FEE_PDA_SEED;
use crate::errors::FeesError;
use crate::events::SocialFeePdaCreated;

// naclac bans on-chain `find_program_address`, so `social_fee_pda`'s seeds
// (dynamic on `user_id`/`platform`) need an explicit, client-computed bump.
// Seed formula confirmed via fees-06#4 against the real bytecode:
// [b"social-fee-pda", user_id.as_bytes(), platform].
#[derive(Accounts)]
#[instruction(user_id: ZcString, platform: u8, social_fee_pda_bump: u8)]
pub struct CreateSocialFeePda {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [SOCIAL_FEE_PDA_SEED, user_id.as_bytes(), &[platform]],
        bump = social_fee_pda_bump
    )]
    pub social_fee_pda: Account<SocialFeePda>,
    pub system_program: Program<System>,
    pub fee_program_global: Account<FeeProgramGlobal>,
}

pub fn create_social_fee_pda(
    ctx: Context<CreateSocialFeePda>,
    user_id: ZcString,
    platform: u8,
    social_fee_pda_bump: u8,
) -> Result {
    // fees-06#8, bit 0: "CreateSocialFeePda is currently disabled".
    require!(
        ctx.accounts.fee_program_global.disable_flags & 0x01 == 0,
        FeesError::FeatureDeactivated
    );
    // fees-02: real storage is 4-byte length prefix + 20-byte content.
    require!(user_id.len() <= 20, FeesError::UserIdTooLong);

    let timestamp = unix_timestamp()?;
    let pda = &mut ctx.accounts.social_fee_pda;
    pda.bump = social_fee_pda_bump;
    pda.version = 1;
    pda.user_id[..user_id.len()].copy_from_slice(user_id.as_bytes());
    pda.user_id_len = user_id.len() as u32;
    pda.platform = platform;

    emit!(SocialFeePdaCreated {
        timestamp,
        user_id: String::from(user_id.as_str()),
        platform,
        social_fee_pda: ctx.accounts.social_fee_pda.address(),
        created_by: ctx.accounts.payer.address(),
    });

    Ok(())
}
