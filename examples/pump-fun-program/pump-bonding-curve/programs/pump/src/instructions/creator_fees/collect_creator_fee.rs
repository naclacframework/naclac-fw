use naclac_lang::prelude::*;
use crate::constants::{CREATOR_VAULT_SEED, PUMP_FEES_PROGRAM_ID};
use crate::errors::PumpError;
use crate::events::CollectCreatorFeeEvent;

// Real accounts (`pump-public-docs/idl/pump.json`): `creator(mut)`,
// `creator_vault(mut, pda=[creator-vault, creator])`, `system_program`. No
// signer anywhere in the real account list — permissionless, same crank
// pattern `distribute_creator_fees` already uses in this program. Real
// bytecode also rejects a `creator` owned by `pump_fees` (i.e. `creator` is
// actually a migrated bonding curve's own `sharing_config` address) with
// `UnableToDistributeCreatorVaultMigratedToSharingConfig` — confirmed live
// via `reference/fee-tier-probe/src/bin/probe72.rs`, not documented in the
// real IDL. Without this check, this instruction would sweep a
// fee-sharing-enabled creator_vault's full balance to the `sharing_config`
// account itself (unspendable there) instead of going through
// `distribute_creator_fees(_v2)`'s proper shareholder split.
#[derive(Accounts)]
#[instruction(creator_vault_bump: u8)]
pub struct CollectCreatorFee {
    /// SAFETY: only a native-lamport transfer destination and PDA seed
    /// material for `creator_vault` below; never deserialized.
    #[account(mut)]
    pub creator: AccountInfo,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data) — only ever a lamport source
    /// below via a signed System Program transfer, never deserialized.
    #[account(
        mut,
        seeds = [CREATOR_VAULT_SEED, creator.address().as_ref()],
        bump = creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,

    pub system_program: Program<System>,
}

/// Sweeps `creator_vault` down to its rent-exempt minimum, paying the full
/// swept amount to `creator` — the native-SOL sibling of
/// `distribute_creator_fees`'s own rent-exempt-floor sweep, just without a
/// shareholder split (100% to `creator`). Permissionless: anyone may crank
/// it, funds always land at the fixed `creator_vault` -> `creator` PDA pair.
pub fn collect_creator_fee(
    ctx: Context<CollectCreatorFee>,
    creator_vault_bump: u8,
) -> Result<Option<CollectCreatorFeeEvent>> {
    require!(
        ctx.accounts.creator.owner() != PUMP_FEES_PROGRAM_ID,
        PumpError::UnableToDistributeCreatorVaultMigratedToSharingConfig
    );

    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(0)?;
    let vault_lamports = ctx.accounts.creator_vault.lamports();
    let available = vault_lamports.saturating_sub(rent_exempt_minimum);

    if available == 0 {
        return Ok(None);
    }

    let creator_address = ctx.accounts.creator.address();
    let signer_seeds: &[&[u8]] = &[CREATOR_VAULT_SEED, creator_address.as_ref(), &[creator_vault_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.system_program.transfer_signed(
        SystemTransferAccounts { from: &mut ctx.accounts.creator_vault, to: &mut ctx.accounts.creator },
        available,
        signer,
    )?;

    let timestamp = unix_timestamp()?;
    let event = emit!(CollectCreatorFeeEvent {
        timestamp,
        creator: creator_address,
        creator_fee: available,
        quote_mint: Address::default(),
    });

    msg!("Creator fee successfully collected");
    Ok(Some(event))
}
