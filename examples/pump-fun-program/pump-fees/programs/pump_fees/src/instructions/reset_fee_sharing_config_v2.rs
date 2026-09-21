use naclac_lang::prelude::*;
use pump_amm_client::instructions::{TransferCreatorFeesToPumpV2Cpi, TransferCreatorFeesToPumpV2CpiAccounts};
use pump_amm_client::PumpAmm;
use pump_client::instructions::{DistributeCreatorFeesV2Cpi, DistributeCreatorFeesV2CpiAccounts, DistributeCreatorFeesV2CpiCall};
use pump_client::Pump;
use crate::components::{BondingCurve, Global, Shareholder, SharingConfig};
use crate::constants::{
    AMM_CREATOR_VAULT_AUTHORITY_SEED, BONDING_CURVE_SEED, PUMP_AMM_PROGRAM_ID, PUMP_CREATOR_VAULT_SEED,
    PUMP_FEES_AUTHORITY_SEED, PUMP_GLOBAL_SEED, PUMP_PROGRAM_ID, SHARING_CONFIG_SEED,
};
use crate::errors::FeesError;
use crate::events::ResetFeeSharingConfigEvent;

// Generalized (any-quote-mint) sibling of `reset_fee_sharing_config`. Same
// `authority == global.admin_set_creator_authority` authorization source as v1
// (per `probe11.rs`; not independently re-probed for v2 specifically, but the
// shared payout-handler finding there makes it certain). `authority` is
// writable here because it doubles as the `payer` for the nested
// `distribute_creator_fees_v2` CPI (needed for its dead-but-declared
// ATA-creation path).
#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, pump_creator_vault_bump: u8, coin_creator_vault_authority_bump: u8)]
pub struct ResetFeeSharingConfigV2 {
    /// SAFETY: only recorded as the sole post-reset shareholder; never read or invoked.
    pub new_admin: AccountInfo,

    #[account(mut)]
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

    /// SAFETY: only touched by the non-native-quote path, which this scoped
    /// pass doesn't implement.
    #[account(mut)]
    pub pump_creator_vault_ata: AccountInfo,

    pub system_program: Program<System>,
    pub pump_program: Program<Pump>,
    pub pump_amm_program: Program<PumpAmm>,

    /// SAFETY: only checked against `WSOL_MINT` inside the nested CPI targets
    /// below; the non-native path isn't implemented in this scoped pass.
    pub quote_mint: AccountInfo,
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
    /// `pump_amm::transfer_creator_fees_to_pump_v2` CPI target, never read here.
    #[account(mut)]
    pub coin_creator_vault_ata: AccountInfo,

    /// SAFETY: only used as the signed-CPI proof-of-origin for
    /// `pump::distribute_creator_fees_v2` below — the `seeds`/`bump`
    /// constraint already verifies its address, and it's never itself a
    /// signer of *this* instruction (it's signed by us, via our own seeds,
    /// only on the outgoing CPI).
    #[account(seeds = [PUMP_FEES_AUTHORITY_SEED], bump)]
    pub pump_fees_authority: AccountInfo,
}

/// Same confirmed mechanics as `reset_fee_sharing_config`, generalized to any
/// quote mint: sweeps AMM-side coin-creator fees, distributes the vault's full
/// pending balance to the *current* shareholder list, then overwrites
/// `sharing_config` with a single 100%-share `new_admin` entry and bumps its
/// version to 2.
pub fn reset_fee_sharing_config_v2(
    ctx: Context<ResetFeeSharingConfigV2>,
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

    ctx.accounts.pump_amm_program.transfer_creator_fees_to_pump_v2(
        TransferCreatorFeesToPumpV2CpiAccounts {
            payer: ctx.accounts.authority.to_cpi_handle_mut(),
            quote_mint: ctx.accounts.quote_mint.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
            coin_creator: ctx.accounts.sharing_config.to_cpi_handle(),
            coin_creator_vault_authority: ctx.accounts.coin_creator_vault_authority.to_cpi_handle_mut(),
            coin_creator_vault_ata: ctx.accounts.coin_creator_vault_ata.to_cpi_handle_mut(),
            pump_creator_vault: ctx.accounts.pump_creator_vault.to_cpi_handle_mut(),
            pump_creator_vault_ata: ctx.accounts.pump_creator_vault_ata.to_cpi_handle_mut(),
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
    ctx.accounts.pump_program.distribute_creator_fees_v2_with_remaining_accounts(
        DistributeCreatorFeesV2CpiCall {
            accounts: DistributeCreatorFeesV2CpiAccounts {
                payer: ctx.accounts.authority.to_cpi_handle_mut(),
                mint: ctx.accounts.mint.to_cpi_handle(),
                bonding_curve: ctx.accounts.bonding_curve.to_cpi_handle(),
                sharing_config: ctx.accounts.sharing_config.to_cpi_handle(),
                creator_vault: ctx.accounts.pump_creator_vault.to_cpi_handle_mut(),
                system_program: ctx.accounts.system_program.to_cpi_handle(),
                creator_vault_quote_token_account: ctx.accounts.pump_creator_vault_ata.to_cpi_handle_mut(),
                quote_mint: ctx.accounts.quote_mint.to_cpi_handle(),
                quote_token_program: ctx.accounts.token_program.to_cpi_handle(),
                associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
                pump_fees_authority: ctx.accounts.pump_fees_authority.to_cpi_handle(),
            },
            bonding_curve_bump,
            creator_vault_bump: pump_creator_vault_bump,
            initialize_ata: Bool::from(false),
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
    let new_shareholders = vec![Shareholder { address: new_admin, share_bps: 10_000, ..Default::default() }];

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
