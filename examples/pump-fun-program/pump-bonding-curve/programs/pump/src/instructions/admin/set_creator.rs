use naclac_lang::prelude::*;
use crate::components::{BondingCurve, Global};
use crate::constants::{BONDING_CURVE_SEED, GLOBAL_SEED, METADATA_SEED, MPL_TOKEN_METADATA_PROGRAM_ID};
use crate::errors::PumpError;
use crate::events::SetCreatorEvent;
use crate::systems::metadata_cpi::read_first_metaplex_creator;

// `metadata_bump`/`bonding_curve_bump` don't exist in the real IDL -- same
// framework-required divergence as `set_metaplex_creator.rs`/`admin_set_creator.rs`.
#[derive(Accounts)]
#[instruction(creator: Address, metadata_bump: u8, bonding_curve_bump: u8)]
pub struct SetCreator {
    pub set_creator_authority: Signer,

    #[account(seeds = [GLOBAL_SEED], bump, set_creator_authority = set_creator_authority @ PumpError::NotAuthorized)]
    pub global: Account<Global>,

    /// SAFETY: only used as a seed input for `metadata`/`bonding_curve`
    /// below, never read or written.
    pub mint: AccountInfo,

    /// SAFETY: the `owner`/`seeds`/`bump` constraints below already prove
    /// this is the real, canonical Metaplex `Metadata` PDA for `mint` —
    /// only its foreign, non-naclac Borsh-encoded data content (not a
    /// naclac component, so not eligible for `Account<T>`'s own typed
    /// validation) is read manually via `read_first_metaplex_creator`.
    #[account(
        owner = MPL_TOKEN_METADATA_PROGRAM_ID,
        seeds = [METADATA_SEED, MPL_TOKEN_METADATA_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        seeds::program = MPL_TOKEN_METADATA_PROGRAM_ID,
        bump = metadata_bump,
    )]
    pub metadata: AccountInfo,

    #[account(mut, seeds = [BONDING_CURVE_SEED, mint.address().as_ref()], bump = bonding_curve_bump)]
    pub bonding_curve: Account<BondingCurve>,
}

// Confirmed exactly against real `pump.so` (`reference/fee-tier-probe/src/bin/probe69.rs`):
// no-ops if `bonding_curve.creator` is already set (logs "Bonding curve has
// creator already set", same as `set_metaplex_creator`). Otherwise, if the
// metadata's `creators` has an entry, `metadata.creators[0].address` wins
// outright -- the `creator` arg is completely ignored in that case (confirmed
// with two different arg values against the same metadata, both producing
// the identical metadata-derived result). Only when metadata's `creators` is
// `None`/empty does the raw `creator` arg get written, literally -- including
// writing `Address::default()` if that's what's passed; there is no
// zero-arg-as-sentinel special case.
pub fn set_creator(
    ctx: Context<SetCreator>,
    creator: Address,
    _metadata_bump: u8,
    _bonding_curve_bump: u8,
) -> Result {
    if ctx.accounts.bonding_curve.creator != Address::default() {
        msg!("Bonding curve has creator already set");
        return Ok(());
    }

    let metadata_creator = {
        let data = ctx.accounts.metadata.try_borrow_data()?;
        read_first_metaplex_creator(data)
    };
    let final_creator = metadata_creator.unwrap_or(creator);

    ctx.accounts.bonding_curve.creator = final_creator;

    let timestamp = unix_timestamp()?;
    emit!(SetCreatorEvent {
        timestamp,
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        creator: final_creator,
    });

    msg!("Creator successfully set");
    Ok(())
}
