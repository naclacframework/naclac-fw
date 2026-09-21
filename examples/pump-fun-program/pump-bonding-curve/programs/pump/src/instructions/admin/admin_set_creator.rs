use naclac_lang::prelude::*;
use crate::components::{BondingCurve, Global};
use crate::constants::{BONDING_CURVE_SEED, GLOBAL_SEED};
use crate::errors::PumpError;
use crate::events::AdminSetCreatorEvent;

#[derive(Accounts)]
#[instruction(creator: Address, bonding_curve_bump: u8)]
pub struct AdminSetCreator {
    pub admin_set_creator_authority: Signer,

    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    /// SAFETY: only used as a seed input for `bonding_curve` below, never
    /// read or written -- a wrong value just fails that seed check.
    pub mint: AccountInfo,

    #[account(mut, seeds = [BONDING_CURVE_SEED, mint.address().as_ref()], bump = bonding_curve_bump)]
    pub bonding_curve: Account<BondingCurve>,
}

pub fn admin_set_creator(
    ctx: Context<AdminSetCreator>,
    creator: Address,
    _bonding_curve_bump: u8,
) -> Result {
    require!(
        ctx.accounts.admin_set_creator_authority.address()
            == ctx.accounts.global.admin_set_creator_authority,
        PumpError::NotAuthorized
    );

    let old_creator = ctx.accounts.bonding_curve.creator;
    ctx.accounts.bonding_curve.creator = creator;

    let timestamp = unix_timestamp()?;
    emit!(AdminSetCreatorEvent {
        timestamp,
        admin_set_creator_authority: ctx.accounts.admin_set_creator_authority.address(),
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        old_creator,
        new_creator: creator,
    });

    msg!("Bonding curve creator successfully set by admin");
    Ok(())
}
