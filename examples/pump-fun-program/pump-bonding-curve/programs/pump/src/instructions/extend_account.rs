use naclac_lang::prelude::*;
use crate::components::{
    BondingCurve, Global, GlobalVolumeAccumulator, SharingConfig, UserVolumeAccumulator,
};
use crate::events::ExtendAccountEvent;

#[derive(Accounts)]
pub struct ExtendAccount {
    /// SAFETY: never deserialized as a typed component -- that's the whole
    /// point (a still-undersized account would fail a typed load before
    /// this instruction's `realloc` constraint ever ran). The `owner`
    /// constraint is the only real validation of the account itself;
    /// `realloc::any_of` identifies its type from its own raw discriminator
    /// and resizes it to that type's current compiled size.
    #[account(
        mut,
        owner = crate::ID,
        realloc::any_of = [Global, BondingCurve, SharingConfig, GlobalVolumeAccumulator, UserVolumeAccumulator],
        realloc::grow_only = true,
        realloc::payer = user,
    )]
    pub account: AccountInfo,

    #[account(mut)]
    pub user: Signer,

    pub system_program: Program<System>,
}

/// Grows a pump-owned account's data buffer to match its currently
/// compiled-in size, funding the extra rent from `user`. `account` stays a
/// raw `AccountInfo` (never a typed `Account<T>`) deliberately: a
/// still-undersized account -- the very thing being extended -- fails
/// `Account<T>`'s own size check before this instruction's `realloc`
/// constraint (let alone its body) ever runs (confirmed empirically:
/// `AccountDataTooSmall` from a typed field against this exact scenario).
/// Recognizes every component this program itself owns; add a new type to
/// the `realloc::any_of` list above when a new component needs the same
/// treatment.
#[instruction]
pub fn extend_account(ctx: Context<ExtendAccount>) -> Result {
    let current_size = ctx.accounts.account.try_borrow_data()?.len();
    let new_size = ctx.bumps.__realloc_space_account;

    let timestamp = unix_timestamp()?;
    emit!(ExtendAccountEvent {
        account: ctx.accounts.account.address(),
        user: ctx.accounts.user.address(),
        current_size: current_size as u64,
        new_size: new_size as u64,
        timestamp,
    });

    msg!("Account successfully extended");
    Ok(())
}
