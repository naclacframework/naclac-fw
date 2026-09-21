use naclac_lang::prelude::*;
use crate::components::{GlobalVolumeAccumulator, UserVolumeAccumulator};
use crate::constants::{GLOBAL_VOLUME_ACCUMULATOR_SEED, USER_VOLUME_ACCUMULATOR_SEED};
use crate::errors::PumpError;
use crate::events::ClaimTokenIncentivesEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `user` (plain, not a
// signer), `user_ata(mut, ATA[user, token_program, mint])` -- unlike
// `claim_cashback_v2`, a dedicated `payer(signer, mut)` is present here, so
// `user_ata` is `init_if_needed`, matching that `payer` funding it for a
// user's first-ever claim, `global_volume_accumulator(pda, singleton,
// read-only)`, `global_incentive_token_account(mut, ATA[
// global_volume_accumulator, token_program, mint])` (the actual token pool
// this sweeps from), `user_volume_accumulator(mut, pda=[seed, user])`,
// `mint` (relation: `global_volume_accumulator.mint`), `token_program`
// (generic), `system_program`, `associated_token_program`, `payer(signer,
// mut)`.
#[instruction_args]
pub struct ClaimTokenIncentivesArgs {
    pub global_incentive_token_account_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: ClaimTokenIncentivesArgs)]
pub struct ClaimTokenIncentives {
    /// SAFETY: only PDA seed material for `user_ata`/`user_volume_accumulator`
    /// below; never deserialized.
    pub user: AccountInfo,

    #[account(
        mut,
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = user,
        token::program = token_program,
    )]
    pub user_ata: InterfaceAccount<TokenAccount>,

    #[account(seeds = [GLOBAL_VOLUME_ACCUMULATOR_SEED], bump)]
    pub global_volume_accumulator: Account<GlobalVolumeAccumulator>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = global_volume_accumulator,
        associated_token::bump = args.global_incentive_token_account_bump,
        token::program = token_program,
    )]
    pub global_incentive_token_account: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = user_volume_accumulator.bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,

    #[account(address = global_volume_accumulator.mint)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,
    pub associated_token_program: Program<AssociatedToken>,

    #[account(mut)]
    pub payer: Signer,
}

/// Sweeps `user_volume_accumulator`'s full `total_unclaimed_tokens` balance
/// out of the shared `global_incentive_token_account` pool into `user_ata`,
/// signed by `global_volume_accumulator`'s own literal-seed PDA authority
/// (same `bare bump` + `ctx.bumps.*` idiom already established for
/// `pump_fees_authority` elsewhere in this program). Mirrors
/// `collect_creator_fee_v2`'s full-balance-sweep shape (no partial claims).
pub fn claim_token_incentives(ctx: Context<ClaimTokenIncentives>, _args: ClaimTokenIncentivesArgs) -> Result<Option<ClaimTokenIncentivesEvent>> {
    let amount = ctx.accounts.user_volume_accumulator.total_unclaimed_tokens;
    if amount == 0 {
        return Ok(None);
    }

    let signer_seeds: &[&[u8]] = &[GLOBAL_VOLUME_ACCUMULATOR_SEED, &[ctx.bumps.global_volume_accumulator]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let mint_decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.global_incentive_token_account,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.user_ata,
            authority: &ctx.accounts.global_volume_accumulator,
        },
        amount,
        mint_decimals,
        signer,
    )?;

    ctx.accounts.user_volume_accumulator.total_unclaimed_tokens = 0;
    ctx.accounts.user_volume_accumulator.total_claimed_tokens = ctx
        .accounts
        .user_volume_accumulator
        .total_claimed_tokens
        .checked_add(amount)
        .ok_or(PumpError::MathOverflow)?;

    let timestamp = unix_timestamp()?;
    let event = emit!(ClaimTokenIncentivesEvent {
        user: ctx.accounts.user.address(),
        mint: ctx.accounts.mint.address(),
        amount,
        timestamp,
        total_claimed_tokens: ctx.accounts.user_volume_accumulator.total_claimed_tokens,
        current_sol_volume: ctx.accounts.user_volume_accumulator.current_sol_volume,
    });

    msg!("Token incentives successfully claimed");
    Ok(Some(event))
}
