use naclac_lang::prelude::*;
use crate::components::{FeeProgramGlobal, SocialFeePda};
use crate::constants::SOCIAL_FEE_PDA_SEED;
use crate::errors::FeesError;
use crate::events::SocialFeePdaClaimed;
use crate::systems::accumulate_claimed;

#[derive(Accounts)]
#[instruction(user_id: ZcString, platform: u8)]
pub struct ClaimSocialFeePda {
    /// SAFETY: only a lamport-transfer destination; not deserialized.
    #[account(mut)]
    pub recipient: AccountInfo,
    #[account(
        mut,
        seeds = [SOCIAL_FEE_PDA_SEED, user_id.as_bytes(), &[platform]],
        bump = social_fee_pda.bump
    )]
    pub social_fee_pda: Account<SocialFeePda>,
    pub fee_program_global: Account<FeeProgramGlobal>,
    pub social_claim_authority: Signer,
}

#[instruction]
pub fn claim_social_fee_pda(
    ctx: Context<ClaimSocialFeePda>,
    user_id: ZcString,
    platform: u8,
) -> Result<Option<SocialFeePdaClaimed>> {
    require!(
        ctx.accounts.social_claim_authority.address()
            == ctx.accounts.fee_program_global.social_claim_authority,
        FeesError::NotAuthorized
    );
    // fees-06#8, bit 1: "ClaimSocialFeePda is currently disabled".
    require!(
        ctx.accounts.fee_program_global.disable_flags & 0x02 == 0,
        FeesError::FeatureDeactivated
    );

    let now = unix_timestamp()?;
    let pda = &mut ctx.accounts.social_fee_pda;

    // fees-06#9: SocialFeePda's rate limit is `>=` (inclusive) and a SOFT
    // failure — log and return Ok(None), no event, no revert.
    let elapsed = now.saturating_sub(pda.last_claimed as i64).max(0) as u64;
    if elapsed < ctx.accounts.fee_program_global.claim_rate_limit {
        msg!("Claim rate limit exceeded");
        return Ok(None);
    }

    // fees-06#14: drain the full native SOL balance down to rent-exemption.
    let data_len = 8 + core::mem::size_of::<SocialFeePda>();
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(data_len)?;
    let claimable_before = ctx.accounts.social_fee_pda.to_account_info().lamports();
    let amount_claimed = claimable_before.saturating_sub(rent_exempt_minimum);
    let recipient_balance_before = ctx.accounts.recipient.lamports();

    if amount_claimed > 0 {
        ctx.accounts.social_fee_pda.sub_lamports(amount_claimed)?;
        ctx.accounts.recipient.add_lamports(amount_claimed)?;
    }

    let pda = &mut ctx.accounts.social_fee_pda;
    pda.total_claimed = accumulate_claimed(pda.total_claimed, amount_claimed)?;
    pda.last_claimed = now as u64;
    let lifetime_claimed = pda.total_claimed;

    let recipient_balance_after = ctx.accounts.recipient.lamports();

    let event = emit!(SocialFeePdaClaimed {
        timestamp: now,
        user_id: String::from(user_id.as_str()),
        platform,
        social_fee_pda: ctx.accounts.social_fee_pda.address(),
        recipient: ctx.accounts.recipient.address(),
        social_claim_authority: ctx.accounts.social_claim_authority.address(),
        amount_claimed,
        claimable_before,
        lifetime_claimed,
        recipient_balance_before,
        recipient_balance_after,
        quote_mint: Address::default(),
    });

    Ok(Some(event))
}
