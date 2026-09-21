use naclac_lang::prelude::*;
use crate::components::UserVolumeAccumulator;
use crate::constants::USER_VOLUME_ACCUMULATOR_SEED;
use crate::errors::PumpError;
use crate::events::ClaimCashbackEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `user(mut)` (plain, not
// a signer -- anyone may crank a payout to any user), `user_volume_accumulator
// (mut, pda=[seed, user])`, `system_program`. No args, no custom errors
// observed anywhere in the real error list for this instruction.
#[derive(Accounts)]
pub struct ClaimCashback {
    /// SAFETY: only a native-lamport transfer destination and PDA seed
    /// material for `user_volume_accumulator` below; never deserialized.
    #[account(mut)]
    pub user: AccountInfo,

    #[account(
        mut,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = user_volume_accumulator.bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,

    pub system_program: Program<System>,
}

/// Sweeps `user_volume_accumulator`'s native-SOL cashback balance down to
/// its rent-exempt minimum, paying the swept amount to `user` -- confirmed
/// mechanism (`fees-07-donation-relay-progress.md`'s `probe16`): cashback
/// lives as real native lamports directly in `user_volume_accumulator`'s
/// own account balance (topped up by `buy`/`sell` on cashback-enabled
/// coins); `cashback_earned` is only a running-total tracker, not itself
/// spendable. `user_volume_accumulator` is owned by this program, so the
/// debit is a direct `sub_lamports` (no signed CPI needed, unlike
/// `creator_vault`'s System-owned sweep in `distribute_creator_fees`).
/// Permissionless, matching the real account list (no signer requirement
/// on `user`).
pub fn claim_cashback(ctx: Context<ClaimCashback>) -> Result<Option<ClaimCashbackEvent>> {
    let data_len = 8 + core::mem::size_of::<UserVolumeAccumulator>();
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(data_len)?;
    let uva_lamports = ctx.accounts.user_volume_accumulator.to_account_info().lamports();
    let amount = uva_lamports.saturating_sub(rent_exempt_minimum);

    if amount == 0 {
        return Ok(None);
    }

    ctx.accounts.user_volume_accumulator.sub_lamports(amount)?;
    ctx.accounts.user.add_lamports(amount)?;

    ctx.accounts.user_volume_accumulator.total_cashback_claimed = ctx
        .accounts
        .user_volume_accumulator
        .total_cashback_claimed
        .checked_add(amount)
        .ok_or(PumpError::MathOverflow)?;

    let timestamp = unix_timestamp()?;
    let event = emit!(ClaimCashbackEvent {
        user: ctx.accounts.user.address(),
        amount,
        timestamp,
        total_claimed: ctx.accounts.user_volume_accumulator.total_cashback_claimed,
        total_cashback_earned: ctx.accounts.user_volume_accumulator.cashback_earned,
    });

    msg!("Cashback successfully claimed");
    Ok(Some(event))
}
