//! Adds a member to a treasury; only its admin may.

use {
    crate::{errors::TreasuryError, state::Treasury},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct AddMember {
    pub admin: Signer,

    #[account(mut, admin = admin)]
    pub treasury: Account<Treasury>,
}

pub fn add_member(ctx: Context<AddMember>, member: Address) -> Result {
    let members = &mut ctx.accounts.treasury.members;
    require!(!members.contains(&member), TreasuryError::AlreadyMember);
    require!(!members.is_full(), TreasuryError::MembersFull);
    members.try_push(member)?;
    Ok(())
}
