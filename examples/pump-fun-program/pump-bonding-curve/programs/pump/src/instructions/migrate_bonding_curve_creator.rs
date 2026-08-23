use naclac_lang::prelude::*;
use crate::components::BondingCurve;
use crate::constants::{BONDING_CURVE_SEED, PUMP_FEES_AUTHORITY_SEED, PUMP_FEES_PROGRAM_ID, SHARING_CONFIG_SEED};
use crate::events::MigrateBondingCurveCreatorEvent;

#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, sharing_config_bump: u8)]
pub struct MigrateBondingCurveCreator {
    /// SAFETY: only used as a seed input for `bonding_curve`/`sharing_config` below, never
    /// read or written — a wrong value just fails those seed checks.
    pub mint: AccountInfo,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        bump = bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    /// SAFETY: the `seeds`/`bump`/`seeds::program` constraint already verifies its address;
    /// only the address is used below, never its data, so no further validation is needed.
    #[account(
        seeds = [SHARING_CONFIG_SEED, mint.address().as_ref()],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump = sharing_config_bump,
    )]
    pub sharing_config: AccountInfo,

    /// SAFETY: `signer` + the `seeds`/`seeds::program` constraint together
    /// prove this call was CPI'd (via `invoke_signed`) by `pump_fees` itself
    /// — only that program can ever produce a valid signature for its own
    /// `PUMP_FEES_AUTHORITY_SEED` PDA. This is the entire authorization
    /// model for this instruction; never deserialized.
    #[account(
        signer,
        seeds = [PUMP_FEES_AUTHORITY_SEED],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump,
    )]
    pub pump_fees_authority: AccountInfo,
}

#[instruction]
pub fn migrate_bonding_curve_creator(
    ctx: Context<MigrateBondingCurveCreator>,
    _bonding_curve_bump: u8,
    _sharing_config_bump: u8,
) -> Result {
    let old_creator = ctx.accounts.bonding_curve.creator;
    let new_creator = ctx.accounts.sharing_config.address();
    ctx.accounts.bonding_curve.creator = new_creator;

    let timestamp = unix_timestamp()?;
    emit!(MigrateBondingCurveCreatorEvent {
        timestamp,
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        sharing_config: ctx.accounts.sharing_config.address(),
        old_creator,
        new_creator,
    });

    msg!("Bonding curve creator successfully migrated");
    Ok(())
}
