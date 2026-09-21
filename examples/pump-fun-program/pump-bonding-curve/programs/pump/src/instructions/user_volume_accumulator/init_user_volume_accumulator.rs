use naclac_lang::prelude::*;
use crate::components::UserVolumeAccumulator;
use crate::constants::USER_VOLUME_ACCUMULATOR_SEED;
use crate::events::InitUserVolumeAccumulatorEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `payer(signer, mut)`,
// `user` (plain, not a signer -- pure PDA seed material, anyone may fund
// anyone else's accumulator), `user_volume_accumulator(init, pda=[seed,
// user])`, `system_program`. No args, no custom errors observed anywhere
// in the real error list for this instruction.
#[derive(Accounts)]
#[instruction(user_volume_accumulator_bump: u8)]
pub struct InitUserVolumeAccumulator {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as PDA seed material for `user_volume_accumulator`
    /// below; never read or invoked.
    pub user: AccountInfo,

    #[account(
        init,
        payer = payer,
        seeds = [USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        bump = user_volume_accumulator_bump,
    )]
    pub user_volume_accumulator: Account<UserVolumeAccumulator>,

    pub system_program: Program<System>,
}

/// Creates a brand-new, zeroed `UserVolumeAccumulator` PDA for `user`,
/// funded by `payer` -- permissionless, matching the real program's own
/// account list (no signer requirement on `user` at all).
pub fn init_user_volume_accumulator(
    ctx: Context<InitUserVolumeAccumulator>,
    user_volume_accumulator_bump: u8,
) -> Result {
    let user_volume_accumulator = &mut ctx.accounts.user_volume_accumulator;
    user_volume_accumulator.user = ctx.accounts.user.address();
    user_volume_accumulator.bump = user_volume_accumulator_bump;

    let timestamp = unix_timestamp()?;
    emit!(InitUserVolumeAccumulatorEvent {
        payer: ctx.accounts.payer.address(),
        user: ctx.accounts.user.address(),
        timestamp,
    });

    msg!("User volume accumulator successfully initialized");
    Ok(())
}
