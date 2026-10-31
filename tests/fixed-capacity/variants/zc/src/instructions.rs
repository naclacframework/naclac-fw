//! Instructions that create the account and change its fixed-capacity fields.

use crate::state::Profile;
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct InitProfile {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"profile"],
        bump
    )]
    pub profile: Account<Profile>,
    pub system_program: Program<System>,
}

pub fn init_profile(ctx: Context<InitProfile>) -> Result {
    let profile = &mut ctx.accounts.profile;
    profile.bump = ctx.bumps.profile;
    profile.owner = ctx.accounts.payer.address();
    Ok(())
}

#[derive(Accounts)]
pub struct Mutate {
    pub signer: Signer,
    #[account(mut, seeds = [b"profile"], bump)]
    pub profile: Account<Profile>,
}

pub fn push_score(ctx: Context<Mutate>, value: u64) -> Result {
    ctx.accounts.profile.scores.try_push(value)?;
    Ok(())
}

pub fn pop_score(ctx: Context<Mutate>) -> Result {
    let popped = ctx.accounts.profile.scores.pop();
    require!(popped.is_some(), NaclacError::InvalidInstructionData);
    Ok(())
}

pub fn set_scores(ctx: Context<Mutate>, values: ZcVec<u64>) -> Result {
    let scores = &mut ctx.accounts.profile.scores;
    scores.clear();
    for value in values.iter() {
        scores.try_push(value)?;
    }
    Ok(())
}

pub fn set_name(ctx: Context<Mutate>, name: ZcString) -> Result {
    ctx.accounts.profile.name.try_set(name.as_str())?;
    Ok(())
}

pub fn add_member(ctx: Context<Mutate>, member: Address) -> Result {
    ctx.accounts.profile.members.try_push(member)?;
    Ok(())
}

pub fn remove_member(ctx: Context<Mutate>, index: u8) -> Result {
    let removed = ctx.accounts.profile.members.remove(index as usize);
    require!(removed.is_some(), NaclacError::InvalidInstructionData);
    Ok(())
}

pub fn push_flag(ctx: Context<Mutate>, value: u8) -> Result {
    ctx.accounts.profile.flags.try_push(Bool::new(value != 0))?;
    Ok(())
}

pub fn bump_counter(ctx: Context<Mutate>) -> Result {
    let profile = &mut ctx.accounts.profile;
    profile.counter = profile.counter.wrapping_add(1);
    Ok(())
}

pub fn clear_all(ctx: Context<Mutate>) -> Result {
    let profile = &mut ctx.accounts.profile;
    profile.scores.clear();
    profile.name.clear();
    profile.members.clear();
    profile.flags.clear();
    Ok(())
}
