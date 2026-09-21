use naclac_lang::prelude::*;
use crate::components::UserVolumeAccumulator;
use crate::constants::USER_VOLUME_ACCUMULATOR_SEED;
use crate::events::CloseUserVolumeAccumulatorEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `user(signer, mut)`,
// `user_volume_accumulator(mut, pda=[seed, user])`, no `system_program` --
// `close = user` moves lamports directly, no CPI needed. Confirmed live
// against real deployed `pump.so` (`reference/fee-tier-probe/src/bin/probe75.rs`):
// genuinely unconditional -- closes successfully regardless of nonzero
// `total_unclaimed_tokens`/`cashback_earned`/`stable_cashback_earned`, no
// error variant gates this anywhere in the real error list. Any pending
// unclaimed tokens or cashback are simply forfeited by the user's own choice
// to close early; the real event itself only reports the volume-tracking
// fields (`total_unclaimed_tokens`/`total_claimed_tokens`/
// `current_sol_volume`/`last_update_timestamp`), not the cashback ones.
#[derive(Accounts)]
pub struct CloseUserVolumeAccumulator {
    #[account(mut)]
    pub user: Signer,

    #[account(
        mut,
        close = user,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = user_volume_accumulator.bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,
}

/// Closes `user`'s own `UserVolumeAccumulator`, reclaiming its rent to
/// `user`. Unconditional -- no precondition on any pending reward field,
/// confirmed against the real deployed program.
pub fn close_user_volume_accumulator(ctx: Context<CloseUserVolumeAccumulator>) -> Result {
    let timestamp = unix_timestamp()?;
    emit!(CloseUserVolumeAccumulatorEvent {
        user: ctx.accounts.user.address(),
        timestamp,
        total_unclaimed_tokens: ctx.accounts.user_volume_accumulator.total_unclaimed_tokens,
        total_claimed_tokens: ctx.accounts.user_volume_accumulator.total_claimed_tokens,
        current_sol_volume: ctx.accounts.user_volume_accumulator.current_sol_volume,
        last_update_timestamp: ctx.accounts.user_volume_accumulator.last_update_timestamp,
    });

    msg!("User volume accumulator successfully closed");
    Ok(())
}
