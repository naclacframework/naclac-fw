use naclac_lang::prelude::*;
use crate::components::BondingCurve;
use crate::constants::{BONDING_CURVE_SEED, METADATA_SEED, MPL_TOKEN_METADATA_PROGRAM_ID};
use crate::events::SetMetaplexCreatorEvent;
use crate::systems::metadata_cpi::read_first_metaplex_creator;

// `metadata_bump`/`bonding_curve_bump` don't exist in the real IDL (real
// Anchor derives PDA bumps on-chain; naclac bans that, so callers must
// precompute and pass them) -- same divergence as `admin_set_creator.rs`'s
// own `bonding_curve_bump` arg.
#[derive(Accounts)]
#[instruction(metadata_bump: u8, bonding_curve_bump: u8)]
pub struct SetMetaplexCreator {
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

// Fully permissionless (no signer anywhere in the real accounts list).
// Confirmed exactly against real `pump.so` (`reference/fee-tier-probe/src/bin/probe69.rs`):
// no-ops if `bonding_curve.creator` is already set (logs "Bonding curve has
// creator already set"), no-ops if the metadata's `creators` is `None`/empty
// (logs "Missing metaplex creators"), otherwise writes `metadata.creators[0].address`
// literally -- confirmed index `[0]` specifically, not whichever entry is
// `verified`. `metadata` must be the exact canonical PDA for `mint` and
// owned by the real Metaplex program, or the call fails outright (confirmed:
// a real, valid, but wrong-mint metadata substitution is rejected).
#[instruction]
pub fn set_metaplex_creator(
    ctx: Context<SetMetaplexCreator>,
    _metadata_bump: u8,
    _bonding_curve_bump: u8,
) -> Result {
    if ctx.accounts.bonding_curve.creator != Address::default() {
        msg!("Bonding curve has creator already set");
        return Ok(());
    }

    let creator = {
        let data = ctx.accounts.metadata.try_borrow_data()?;
        read_first_metaplex_creator(data)
    };
    let Some(creator) = creator else {
        msg!("Missing metaplex creators");
        return Ok(());
    };

    ctx.accounts.bonding_curve.creator = creator;

    let timestamp = unix_timestamp()?;
    emit!(SetMetaplexCreatorEvent {
        timestamp,
        mint: ctx.accounts.mint.address(),
        bonding_curve: ctx.accounts.bonding_curve.address(),
        metadata: ctx.accounts.metadata.address(),
        creator,
    });

    msg!("Metaplex creator successfully set");
    Ok(())
}
