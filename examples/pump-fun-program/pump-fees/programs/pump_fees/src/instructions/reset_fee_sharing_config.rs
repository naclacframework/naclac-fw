use naclac_lang::prelude::*;
use pump_amm_client::instructions::{TransferCreatorFeesToPumpCpi, TransferCreatorFeesToPumpCpiAccounts};
use pump_amm_client::PumpAmm;
use pump_client::instructions::{DistributeCreatorFeesCpi, DistributeCreatorFeesCpiAccounts, DistributeCreatorFeesCpiCall};
use pump_client::Pump;
use crate::components::{BondingCurve, Global, Shareholder, SharingConfig};
use crate::constants::{
    AMM_CREATOR_VAULT_AUTHORITY_SEED, BONDING_CURVE_SEED, PUMP_AMM_PROGRAM_ID, PUMP_CREATOR_VAULT_SEED,
    PUMP_FEES_AUTHORITY_SEED, PUMP_GLOBAL_SEED, PUMP_PROGRAM_ID, SHARING_CONFIG_SEED, WSOL_MINT,
};
use crate::errors::FeesError;
use crate::events::ResetFeeSharingConfigEvent;

// WSOL-only legacy variant, superseded by `reset_fee_sharing_config_v2` for
// non-native quote mints. `authority` must equal `global.admin_set_creator_authority`
// — confirmed empirically via `reference/fee-tier-probe/src/bin/probe11.rs` against
// the real bytecode (distinct from `update_fee_shares(_v2)`, which checks
// `sharing_config.admin` instead, per `probe12.rs`).
#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, pump_creator_vault_bump: u8, coin_creator_vault_authority_bump: u8)]
pub struct ResetFeeSharingConfig {
    /// SAFETY: only recorded as the sole post-reset shareholder; never read or invoked.
    pub new_admin: AccountInfo,

    pub authority: Signer,

    #[account(seeds = [PUMP_GLOBAL_SEED], seeds::program = PUMP_PROGRAM_ID, owner = PUMP_PROGRAM_ID)]
    pub global: Account<Global>,

    /// SAFETY: only used as PDA seed material for `bonding_curve`/`sharing_config`
    /// below, never read or invoked.
    pub mint: AccountInfo,

    #[account(
        mut,
        seeds = [SHARING_CONFIG_SEED, mint.address().as_ref()],
        bump = sharing_config.bump,
    )]
    pub sharing_config: Account<SharingConfig>,

    #[account(
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = bonding_curve_bump,
        owner = PUMP_PROGRAM_ID,
    )]
    pub bonding_curve: Account<BondingCurve>,

    /// SAFETY: the `seeds`/`bump`/`seeds::program` constraint already verifies its
    /// address; it's a lamport-only PDA under `pump` (no stored data from this
    /// program's perspective), only ever a CPI target below, never deserialized.
    #[account(
        mut,
        seeds = [PUMP_CREATOR_VAULT_SEED, sharing_config.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = pump_creator_vault_bump,
    )]
    pub pump_creator_vault: AccountInfo,

    pub system_program: Program<System>,
    pub pump_program: Program<Pump>,
    pub pump_amm_program: Program<PumpAmm>,

    #[account(address = WSOL_MINT)]
    pub wsol_mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,

    /// SAFETY: the `seeds`/`bump`/`seeds::program` constraint already verifies its
    /// address; it's a lamport-only PDA authority under `pump_amm` (no stored data
    /// from this program's perspective), only ever a CPI signer/target below,
    /// never deserialized.
    #[account(
        mut,
        seeds = [AMM_CREATOR_VAULT_AUTHORITY_SEED, sharing_config.address().as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = coin_creator_vault_authority_bump,
    )]
    pub coin_creator_vault_authority: AccountInfo,

    /// SAFETY: only a CPI passthrough — deserialized and mutated by the nested
    /// `pump_amm::transfer_creator_fees_to_pump` CPI target, never read here.
    #[account(mut)]
    pub coin_creator_vault_ata: AccountInfo,

    /// SAFETY: only used as the signed-CPI proof-of-origin for
    /// `pump::distribute_creator_fees` below — the `seeds`/`bump` constraint
    /// already verifies its address, and it's never itself a signer of
    /// *this* instruction (it's signed by us, via our own seeds, only on
    /// the outgoing CPI).
    #[account(seeds = [PUMP_FEES_AUTHORITY_SEED], bump)]
    pub pump_fees_authority: AccountInfo,
}

/// Reset Fee Sharing Config and distribute pending fees first. Sweeps any AMM-side
/// coin-creator fees into the bonding-curve vault, distributes the vault's full
/// pending balance to the *current* shareholder list (one `remaining_accounts`
/// entry per shareholder, in order), then overwrites `sharing_config` with a single
/// 100%-share `new_admin` entry and bumps its version to 2 — confirmed end-to-end
/// via `reference/fee-tier-probe/src/bin/probe11.rs` against the real bytecode.
#[instruction]
pub fn reset_fee_sharing_config(
    ctx: Context<ResetFeeSharingConfig>,
    bonding_curve_bump: u8,
    pump_creator_vault_bump: u8,
    coin_creator_vault_authority_bump: u8,
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.admin_set_creator_authority,
        FeesError::NotAuthorized
    );

    let shareholders_len = ctx.accounts.sharing_config.shareholders_len as usize;
    require!(
        ctx.remaining_accounts.len() == shareholders_len,
        FeesError::NotEnoughRemainingAccounts
    );

    ctx.accounts.pump_amm_program.transfer_creator_fees_to_pump(
        TransferCreatorFeesToPumpCpiAccounts {
            wsol_mint: ctx.accounts.wsol_mint.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
            coin_creator: ctx.accounts.sharing_config.to_cpi_handle(),
            coin_creator_vault_authority: ctx.accounts.coin_creator_vault_authority.to_cpi_handle_mut(),
            coin_creator_vault_ata: ctx.accounts.coin_creator_vault_ata.to_cpi_handle_mut(),
            pump_creator_vault: ctx.accounts.pump_creator_vault.to_cpi_handle_mut(),
        },
        coin_creator_vault_authority_bump,
        pump_creator_vault_bump,
    )?;

    let mut remaining_for_distribute: Vec<(AccountInfo, bool, bool)> = Vec::with_capacity(shareholders_len);
    for account in ctx.remaining_accounts {
        remaining_for_distribute.push((*account, true, false));
    }

    let pump_fees_authority_signer_seeds: &[&[u8]] =
        &[PUMP_FEES_AUTHORITY_SEED, &[ctx.bumps.pump_fees_authority]];
    let pump_fees_authority_signer: &[&[&[u8]]] = &[pump_fees_authority_signer_seeds];
    ctx.accounts.pump_program.distribute_creator_fees_with_remaining_accounts(
        DistributeCreatorFeesCpiCall {
            accounts: DistributeCreatorFeesCpiAccounts {
                mint: ctx.accounts.mint.to_cpi_handle(),
                bonding_curve: ctx.accounts.bonding_curve.to_cpi_handle(),
                sharing_config: ctx.accounts.sharing_config.to_cpi_handle(),
                creator_vault: ctx.accounts.pump_creator_vault.to_cpi_handle_mut(),
                system_program: ctx.accounts.system_program.to_cpi_handle(),
                pump_fees_authority: ctx.accounts.pump_fees_authority.to_cpi_handle(),
            },
            bonding_curve_bump,
            creator_vault_bump: pump_creator_vault_bump,
            signer_seeds: pump_fees_authority_signer,
            remaining_accounts: &remaining_for_distribute,
        },
    )?;

    let timestamp = unix_timestamp()?;
    let sharing_config_address = ctx.accounts.sharing_config.address();
    let mint_address = ctx.accounts.mint.address();
    let old_admin = ctx.accounts.sharing_config.admin;
    let old_shareholders: Vec<Shareholder> =
        ctx.accounts.sharing_config.shareholders[..shareholders_len].to_vec();
    let old_version = ctx.accounts.sharing_config.version;
    let new_admin = ctx.accounts.new_admin.address();
    let new_shareholders = vec![Shareholder { address: new_admin, share_bps: 10_000 }];

    let sharing_config = &mut ctx.accounts.sharing_config;
    sharing_config.admin = new_admin;
    sharing_config.shareholders[0] = new_shareholders[0];
    sharing_config.shareholders_len = 1;
    sharing_config.version = 2;
    let new_version = sharing_config.version;

    emit!(ResetFeeSharingConfigEvent {
        timestamp,
        mint: mint_address,
        sharing_config: sharing_config_address,
        old_admin,
        old_shareholders,
        new_admin,
        new_shareholders,
        old_version,
        new_version,
    });

    Ok(())
}
