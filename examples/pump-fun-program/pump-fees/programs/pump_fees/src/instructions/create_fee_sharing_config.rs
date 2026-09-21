use naclac_lang::prelude::*;
use pump_client::instructions::{MigrateBondingCurveCreatorCpi, MigrateBondingCurveCreatorCpiAccounts};
use pump_client::Pump;
use crate::components::{BondingCurve, Global, Shareholder, SharingConfig, SHARING_CONFIG_STATUS_ACTIVE};
use crate::constants::{
    BONDING_CURVE_SEED, PUMP_FEES_AUTHORITY_SEED, PUMP_GLOBAL_SEED, PUMP_PROGRAM_ID, SHARING_CONFIG_SEED,
};
use crate::errors::FeesError;

#[derive(Accounts)]
#[instruction(bonding_curve_bump: u8, sharing_config_bump: u8)]
pub struct CreateFeeSharingConfig {
    #[account(mut)]
    pub payer: Signer,

    #[account(seeds = [PUMP_GLOBAL_SEED], seeds::program = PUMP_PROGRAM_ID, owner = PUMP_PROGRAM_ID)]
    pub pump_global: Account<Global>,

    /// SAFETY: only used as a seed input for `bonding_curve`/`sharing_config` below, never
    /// read or written — a wrong value just fails those seed checks.
    pub mint: AccountInfo,

    #[account(
        init,
        payer = payer,
        seeds = [SHARING_CONFIG_SEED, mint.address().as_ref()],
        bump = sharing_config_bump,
    )]
    pub sharing_config: Account<SharingConfig>,

    pub system_program: Program<System>,

    #[account(
        mut,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = bonding_curve_bump,
        owner = PUMP_PROGRAM_ID,
    )]
    pub bonding_curve: Account<BondingCurve>,

    pub pump_program: Program<Pump>,

    /// SAFETY: only used as the signed-CPI proof-of-origin for
    /// `pump::migrate_bonding_curve_creator` below — the `seeds`/`bump`
    /// constraint already verifies its address, and it's never itself a
    /// signer of *this* instruction (it's signed by us, via our own seeds,
    /// only on the outgoing CPI).
    #[account(seeds = [PUMP_FEES_AUTHORITY_SEED], bump)]
    pub pump_fees_authority: AccountInfo,
}

pub fn create_fee_sharing_config(
    ctx: Context<CreateFeeSharingConfig>,
    bonding_curve_bump: u8,
    sharing_config_bump: u8,
) -> Result {
    let payer_address = ctx.accounts.payer.address();
    let creator = ctx.accounts.bonding_curve.creator;

    require!(
        payer_address == creator
            || payer_address == ctx.accounts.pump_global.admin_set_creator_authority,
        FeesError::InvalidAdmin
    );

    let sharing_config = &mut ctx.accounts.sharing_config;
    sharing_config.bump = sharing_config_bump;
    sharing_config.version = 2;
    sharing_config.status = SHARING_CONFIG_STATUS_ACTIVE;
    sharing_config.mint = ctx.accounts.mint.address();
    sharing_config.admin = payer_address;
    sharing_config.admin_revoked = 0;
    sharing_config.shareholders[0] = Shareholder {
        address: creator,
        share_bps: 10_000,
        ..Default::default()
    };
    sharing_config.shareholders_len = 1;

    let pump_fees_authority_signer_seeds: &[&[u8]] =
        &[PUMP_FEES_AUTHORITY_SEED, &[ctx.bumps.pump_fees_authority]];
    let pump_fees_authority_signer: &[&[&[u8]]] = &[pump_fees_authority_signer_seeds];
    
    ctx.accounts.pump_program.migrate_bonding_curve_creator_signed(
        MigrateBondingCurveCreatorCpiAccounts {
            mint: ctx.accounts.mint.to_cpi_handle(),
            bonding_curve: ctx.accounts.bonding_curve.to_cpi_handle_mut(),
            sharing_config: ctx.accounts.sharing_config.to_cpi_handle(),
            pump_fees_authority: ctx.accounts.pump_fees_authority.to_cpi_handle(),
        },
        bonding_curve_bump,
        sharing_config_bump,
        pump_fees_authority_signer,
    )?;

    Ok(())
}
