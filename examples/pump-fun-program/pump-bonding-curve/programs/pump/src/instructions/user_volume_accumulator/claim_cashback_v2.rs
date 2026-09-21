use naclac_lang::prelude::*;
use crate::components::UserVolumeAccumulator;
use crate::constants::USER_VOLUME_ACCUMULATOR_SEED;
use crate::errors::PumpError;
use crate::events::ClaimCashbackEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `user(mut)` (plain, not
// a signer), `user_volume_accumulator(mut, pda=[seed, user])`, `quote_mint`,
// `quote_token_program`, `associated_token_program`,
// `associated_user_volume_accumulator(mut, ATA[user_volume_accumulator,
// quote_token_program, quote_mint])` (the actual SPL-token-denominated
// "stable cashback" balance -- `UserVolumeAccumulator` itself is a plain
// Borsh/zero-copy account, it can't hold SPL tokens directly the way it
// holds native-SOL cashback), `associated_quote_user(mut, ATA[user,
// quote_token_program, quote_mint])`, `system_program`. No `payer`/
// `init_if_needed` anywhere in the real list (unlike `claim_token_incentives`,
// which has one) -- both ATAs must already exist, same shape as
// `collect_creator_fee_v2`'s own precedent.
#[instruction_args]
pub struct ClaimCashbackV2Args {
    pub associated_user_volume_accumulator_bump: u8,
    pub associated_quote_user_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: ClaimCashbackV2Args)]
pub struct ClaimCashbackV2 {
    /// SAFETY: only PDA seed material for `user_volume_accumulator`/
    /// `associated_quote_user` below; never deserialized.
    #[account(mut)]
    pub user: AccountInfo,

    #[account(
        mut,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = user_volume_accumulator.bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,

    pub quote_mint: InterfaceAccount<Mint>,
    pub quote_token_program: Interface<TokenInterface>,
    pub associated_token_program: Program<AssociatedToken>,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = user_volume_accumulator,
        associated_token::bump = args.associated_user_volume_accumulator_bump,
        token::program = quote_token_program,
    )]
    pub associated_user_volume_accumulator: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = user,
        associated_token::bump = args.associated_quote_user_bump,
        token::program = quote_token_program,
    )]
    pub associated_quote_user: InterfaceAccount<TokenAccount>,

    pub system_program: Program<System>,
}

/// Sweeps `associated_user_volume_accumulator`'s full SPL-token "stable
/// cashback" balance to `associated_quote_user` -- the SPL-token sibling of
/// `claim_cashback` (native SOL). No rent-exempt floor to preserve (an SPL
/// token account's rent is a fixed lamport reserve separate from its token
/// balance), so the entire balance is swept, matching
/// `collect_creator_fee_v2`'s identical shape. Permissionless (no signer
/// requirement on `user`).
pub fn claim_cashback_v2(ctx: Context<ClaimCashbackV2>, _args: ClaimCashbackV2Args) -> Result<Option<ClaimCashbackEvent>> {
    let amount = ctx.accounts.associated_user_volume_accumulator.amount();
    if amount == 0 {
        return Ok(None);
    }

    let user_address = ctx.accounts.user.address();
    let signer_seeds: &[&[u8]] =
        &[USER_VOLUME_ACCUMULATOR_SEED, user_address.as_ref(), &[ctx.accounts.user_volume_accumulator.bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let quote_decimals = ctx.accounts.quote_mint.decimals();
    ctx.accounts.quote_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.associated_user_volume_accumulator,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.associated_quote_user,
            authority: &ctx.accounts.user_volume_accumulator,
        },
        amount,
        quote_decimals,
        signer,
    )?;

    ctx.accounts.user_volume_accumulator.total_stable_cashback_claimed = ctx
        .accounts
        .user_volume_accumulator
        .total_stable_cashback_claimed
        .checked_add(amount)
        .ok_or(PumpError::MathOverflow)?;

    let timestamp = unix_timestamp()?;
    let event = emit!(ClaimCashbackEvent {
        user: user_address,
        amount,
        timestamp,
        total_claimed: ctx.accounts.user_volume_accumulator.total_stable_cashback_claimed,
        total_cashback_earned: ctx.accounts.user_volume_accumulator.stable_cashback_earned,
    });

    msg!("Cashback successfully claimed");
    Ok(Some(event))
}
