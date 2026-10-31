//! Instructions that create the account and fill its sized fields.

use crate::state::Roster;
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct InitRoster {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"roster"],
        bump
    )]
    pub roster: Account<Roster>,
    pub system_program: Program<System>,
}

pub fn init_roster(ctx: Context<InitRoster>) -> Result {
    ctx.accounts.roster.bump = ctx.bumps.roster;
    Ok(())
}

#[derive(Accounts)]
pub struct Mutate {
    pub signer: Signer,
    #[account(mut, seeds = [b"roster"], bump)]
    pub roster: Account<Roster>,
}

pub fn fill_roster(ctx: Context<Mutate>, count: u8) -> Result {
    let roster = &mut ctx.accounts.roster;
    roster.members.clear();
    for index in 0..count {
        roster.members.push(Address::new_from_array([index + 1; 32]));
    }
    Ok(())
}

pub fn set_label(ctx: Context<Mutate>, label: String) -> Result {
    ctx.accounts.roster.label = label;
    Ok(())
}
