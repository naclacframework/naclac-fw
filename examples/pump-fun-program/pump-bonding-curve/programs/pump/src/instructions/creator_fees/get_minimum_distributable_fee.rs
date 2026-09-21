use naclac_lang::prelude::*;
use crate::components::{BondingCurve, MinimumDistributableFeeEvent, SharingConfig};
use crate::constants::{
    BONDING_CURVE_SEED, CREATOR_VAULT_RENT_EXEMPT_MINIMUM, CREATOR_VAULT_SEED, PUMP_FEES_PROGRAM_ID,
    SHARING_CONFIG_SEED,
};
use crate::errors::PumpError;

// Real accounts (`pump-public-docs/idl/pump.json`): `mint` (relation:
// `sharing_config`), `bonding_curve` (pda), `sharing_config` (pda, owned by
// `pump_fees`), `creator_vault` (pda, from `bonding_curve.creator`). Real
// IDL declares zero args/remaining_accounts, but empirically (`probe71.rs`
// against real deployed `pump.so`) this instruction genuinely shares its
// core logic with `distribute_creator_fees` internally (the real bytecode's
// own panic trace names `programs/pump/src/creator/distribute_creator_fees.rs`)
// and requires the exact same `remaining_accounts` shape: one entry per
// `sharing_config.shareholders`, address-matched in order
// (`ShareholderAccountMismatch` on a mismatch, confirmed live), plus a
// dedicated `bonding_curve.creator == sharing_config` relation check
// (`BondingCurveAndSharingConfigCreatorMismatch`, confirmed live) — neither
// is documented anywhere in the real IDL. `bonding_curve.creator` equaling
// `sharing_config`'s own PDA address (not a human pubkey) is itself the
// confirmed post-`create_fee_sharing_config` invariant this check enforces.
#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, creator_vault_bump: u8)]
pub struct GetMinimumDistributableFee {
    /// SAFETY: only used as PDA seed material for `bonding_curve`/`sharing_config`
    /// below, never read or invoked.
    pub mint: AccountInfo,

    #[account(seeds = [BONDING_CURVE_SEED, mint.address().as_ref()], bump = bonding_curve_bump)]
    pub bonding_curve: Account<BondingCurve>,

    #[account(
        seeds = [SHARING_CONFIG_SEED, mint.address().as_ref()],
        seeds::program = PUMP_FEES_PROGRAM_ID,
        bump = sharing_config.bump,
        owner = PUMP_FEES_PROGRAM_ID,
    )]
    pub sharing_config: Account<SharingConfig>,

    /// SAFETY: the `seeds`/`bump` constraint already verifies its address;
    /// it's a lamport-only PDA (no stored data), never deserialized — only
    /// its raw lamport balance is read.
    #[account(
        seeds = [CREATOR_VAULT_SEED, bonding_curve.creator.as_ref()],
        bump = creator_vault_bump,
    )]
    pub creator_vault: AccountInfo,
}

/// Permissionless view instruction (confirmed via real IDL doc comment) that
/// reports whether `creator_vault` currently holds enough to be worth
/// cranking via `distribute_creator_fees(_v2)`. Real formula, empirically
/// confirmed against real deployed `pump.so` (`reference/fee-tier-probe/src/bin/probe71.rs`,
/// not inferred): `minimum_required` is a constant `2 *
/// CREATOR_VAULT_RENT_EXEMPT_MINIMUM`; `distributable_fees` is
/// `creator_vault`'s raw lamport balance down to the single rent-exempt
/// floor (matching `distribute_creator_fees`'s own calculation exactly);
/// `can_distribute` compares the *raw* `creator_vault` balance (not
/// `distributable_fees`) against `minimum_required`, inclusive (`>=`). Like
/// `pump_fees::get_fees`, this is a plain returned value, never `emit!`'d,
/// despite the real IDL naming the return type `*Event`.
pub fn get_minimum_distributable_fee(
    ctx: Context<GetMinimumDistributableFee>,
    _bonding_curve_bump: u8,
    _creator_vault_bump: u8,
) -> Result<MinimumDistributableFeeEvent> {
    require!(
        ctx.accounts.bonding_curve.creator == ctx.accounts.sharing_config.address(),
        PumpError::BondingCurveAndSharingConfigCreatorMismatch
    );

    let shareholders_len = ctx.accounts.sharing_config.shareholders_len as usize;
    require!(shareholders_len > 0, PumpError::NotEnoughRemainingAccounts);
    require!(
        ctx.remaining_accounts.len() == shareholders_len,
        PumpError::NotEnoughRemainingAccounts
    );
    for i in 0..shareholders_len {
        let shareholder = ctx.accounts.sharing_config.shareholders[i];
        require!(
            ctx.remaining_accounts[i].address() == shareholder.address,
            PumpError::ShareholderAccountMismatch
        );
    }

    let minimum_required = 2 * CREATOR_VAULT_RENT_EXEMPT_MINIMUM;
    let vault_lamports = ctx.accounts.creator_vault.lamports();
    let distributable_fees = vault_lamports.saturating_sub(CREATOR_VAULT_RENT_EXEMPT_MINIMUM);
    let can_distribute = vault_lamports >= minimum_required;

    msg!("Minimum distributable fee successfully computed");
    Ok(MinimumDistributableFeeEvent {
        minimum_required,
        distributable_fees,
        can_distribute: Bool::from(can_distribute),
        ..Default::default()
    })
}
