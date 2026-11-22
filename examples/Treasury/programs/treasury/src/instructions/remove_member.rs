//! Removes a member from a treasury; only its admin may, and never below the
//! threshold.

use {
    crate::{errors::TreasuryError, state::Treasury},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct RemoveMember {
    pub admin: Signer,

    #[account(mut, admin = admin)]
    pub treasury: Account<Treasury>,
}

pub fn remove_member(ctx: Context<RemoveMember>, member: Address) -> Result {
    let treasury = &mut ctx.accounts.treasury;
    let Some(index) = treasury
        .members
        .iter()
        .position(|existing| existing == member)
    else {
        return Err(TreasuryError::NotAMember.into());
    };
    require!(
        treasury.members.len() > usize::from(treasury.threshold),
        TreasuryError::BelowThreshold
    );
    treasury.members.remove(index);
    Ok(())
}
