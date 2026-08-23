use naclac_lang::prelude::*;
use crate::components::{FeeProgramGlobal, SocialFeePda};
use crate::constants::SOCIAL_FEE_PDA_SEED;
use crate::errors::FeesError;
use crate::events::SocialFeePdaClaimed;
use crate::systems::accumulate_claimed;

#[derive(Accounts)]
#[instruction(user_id: ZcString, platform: u8)]
pub struct ClaimSocialFeePdaV2 {
    /// SAFETY: only a token-transfer destination owner; not deserialized.
    #[account(mut)]
    pub recipient: AccountInfo,
    #[account(
        mut,
        seeds = [SOCIAL_FEE_PDA_SEED, user_id.as_bytes(), &[platform]],
        bump = social_fee_pda.bump
    )]
    pub social_fee_pda: Account<SocialFeePda>,
    #[account(mut)]
    pub quote_mint: Account<Mint>,
    #[account(mut)]
    pub associated_social_fee_pda: Account<TokenAccount>,
    /// SAFETY: may not exist yet — created idempotently by the
    /// `init_if_needed`/`associated_token::mint`/`::authority` constraint
    /// below (a no-op if it already exists), then reinterpreted as a
    /// `TokenAccount` in the instruction body.
    #[account(
        mut,
        init_if_needed,
        payer = social_claim_authority,
        associated_token::mint = quote_mint,
        associated_token::authority = recipient,
    )]
    pub associated_recipient: AccountInfo,
    pub quote_token_program: Interface<TokenInterface>,
    pub associated_token_program: Program<AssociatedToken>,
    pub fee_program_global: Account<FeeProgramGlobal>,
    #[account(mut)]
    pub social_claim_authority: Signer,
    pub system_program: Program<System>,
}

#[instruction]
pub fn claim_social_fee_pda_v2(
    ctx: Context<ClaimSocialFeePdaV2>,
    user_id: ZcString,
    platform: u8,
) -> Result<Option<SocialFeePdaClaimed>> {
    require!(
        ctx.accounts.social_claim_authority.address()
            == ctx.accounts.fee_program_global.social_claim_authority,
        FeesError::NotAuthorized
    );
    require!(
        ctx.accounts.fee_program_global.disable_flags & 0x02 == 0,
        FeesError::FeatureDeactivated
    );

    let now = unix_timestamp()?;
    let pda = &mut ctx.accounts.social_fee_pda;

    let elapsed = now.saturating_sub(pda.last_claimed as i64).max(0) as u64;
    if elapsed < ctx.accounts.fee_program_global.claim_rate_limit {
        msg!("Claim rate limit exceeded");
        return Ok(None);
    }

    let mut associated_recipient =
        Account::<TokenAccount>::try_from_mut(&ctx.accounts.associated_recipient, 0)?;

    let signer_seeds: &[&[u8]] = &[
        &SOCIAL_FEE_PDA_SEED,
        user_id.as_bytes(),
        &[platform],
        &[pda.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[signer_seeds];

    let claimable_before = ctx.accounts.associated_social_fee_pda.amount();
    let recipient_balance_before = associated_recipient.amount();
    let amount_claimed = claimable_before;
    let quote_decimals = ctx.accounts.quote_mint.decimals();

    if amount_claimed > 0 {
        ctx.accounts.quote_token_program.transfer_checked_signed(
            TransferCheckedAccounts {
                from: &mut ctx.accounts.associated_social_fee_pda,
                mint: &ctx.accounts.quote_mint,
                to: &mut associated_recipient,
                authority: &ctx.accounts.social_fee_pda,
            },
            amount_claimed,
            quote_decimals,
            signer_seeds,
        )?;
    }

    let pda = &mut ctx.accounts.social_fee_pda;
    pda.total_stable_claimed = accumulate_claimed(pda.total_stable_claimed, amount_claimed)?;
    pda.last_claimed = now as u64;
    let lifetime_claimed = pda.total_stable_claimed;

    let recipient_balance_after = associated_recipient.amount();

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
        quote_mint: ctx.accounts.quote_mint.address(),
    });

    Ok(Some(event))
}
