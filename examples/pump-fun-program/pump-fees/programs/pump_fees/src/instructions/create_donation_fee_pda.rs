use naclac_lang::prelude::*;
use crate::components::{BondingCurve, DonationFeePda, FeeProgramGlobal, Pool};
use crate::constants::{
    BONDING_CURVE_SEED, DONATION_FEE_PDA_SEED, FEE_PROGRAM_GLOBAL_SEED, POOL_AUTHORITY_SEED,
    POOL_SEED, PUMP_AMM_PROGRAM_ID, PUMP_PROGRAM_ID, WSOL_MINT,
};
use crate::errors::FeesError;
use crate::events::DonationFeePdaCreated;

// Real pump_fees requires `pool` to be a real, pump_amm-owned account at the
// canonical migrated-pool address whenever `bonding_curve.complete` — but
// empirically (probe: donation-fee-pda-probe/find_pool + find_creation_tx),
// every real `DonationFeePda` ever created is for an UNGRADUATED mint, where
// no such pool exists yet. Reproducing that check unconditionally would make
// this instruction non-functional for its only observed real use case. Here,
// `pool`'s address is still verified unconditionally via seeds (cheap, and
// doesn't require the account to exist) via naclac's own PDA-seed-hash path;
// its owner and `quote_mint` content are validated only when
// `bonding_curve.complete`, since that's the only case where a real pool can
// exist to check at all.
//
// `pool_authority` exists purely as PDA seed material for `pool` — naclac
// bans on-chain `find_program_address`/`create_program_address`, so this
// must be a real Accounts-struct field (seed derivation only works between
// sibling fields), even though its own content is never read.
//
// `sharing_config` is accepted for account-shape parity with the real
// program's IDL but is dead: a real mainnet transaction creating a
// `DonationFeePda` passed an unrelated wallet address in this slot and
// succeeded, proving the real instruction doesn't validate or read it.
#[derive(Accounts)]
#[instruction(
    bonding_curve_bump: u8,
    pool_authority_bump: u8,
    pool_bump: u8,
    donation_fee_pda_bump: u8,
)]
pub struct CreateDonationFeePda {
    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    #[account(seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,

    /// SAFETY: only used as PDA seed material below; not deserialized.
    pub base_mint: AccountInfo,

    #[account(
        seeds = [BONDING_CURVE_SEED, base_mint.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = bonding_curve_bump,
        owner = PUMP_PROGRAM_ID,
    )]
    pub bonding_curve: Account<BondingCurve>,

    /// SAFETY: only used as PDA seed material for `pool` below; not deserialized.
    #[account(
        seeds = [POOL_AUTHORITY_SEED, base_mint.address().as_ref()],
        seeds::program = PUMP_PROGRAM_ID,
        bump = pool_authority_bump,
    )]
    pub pool_authority: AccountInfo,

    /// SAFETY: address is always verified via seeds; owner/content are only
    /// checked when `bonding_curve.complete` (see module comment above).
    #[account(
        seeds = [POOL_SEED, &0u16.to_le_bytes(), pool_authority.address().as_ref(), base_mint.address().as_ref(), WSOL_MINT.as_ref()],
        seeds::program = PUMP_AMM_PROGRAM_ID,
        bump = pool_bump,
    )]
    pub pool: AccountInfo,

    /// SAFETY: dead — see module comment above. Kept for account-shape parity only.
    pub sharing_config: AccountInfo,

    /// SAFETY: plain address material distinguishing donation campaigns for
    /// the same `base_mint`; not a signer, never deserialized.
    pub config_id: AccountInfo,

    #[account(
        init,
        payer = payer,
        seeds = [DONATION_FEE_PDA_SEED, base_mint.address().as_ref(), config_id.address().as_ref()],
        bump = donation_fee_pda_bump,
    )]
    pub donation_fee_pda: Account<DonationFeePda>,
}

pub fn create_donation_fee_pda(
    ctx: Context<CreateDonationFeePda>,
    _bonding_curve_bump: u8,
    _pool_authority_bump: u8,
    _pool_bump: u8,
    donation_fee_pda_bump: u8,
) -> Result {
    let is_complete: bool = ctx.accounts.bonding_curve.complete.into();
    if is_complete {
        require!(
            ctx.accounts.pool.program_owner() == PUMP_AMM_PROGRAM_ID,
            FeesError::InvalidPool
        );
        let pool = Account::<Pool>::try_from(&ctx.accounts.pool, 0)?;
        require!(pool.quote_mint == WSOL_MINT, FeesError::InvalidPool);
    }

    let timestamp = unix_timestamp()?;
    let creator = ctx.accounts.bonding_curve.creator;
    let base_mint_addr = ctx.accounts.base_mint.address();
    let config_id_addr = ctx.accounts.config_id.address();

    let donation_fee_pda = &mut ctx.accounts.donation_fee_pda;
    donation_fee_pda.bump = donation_fee_pda_bump;
    donation_fee_pda.version = 1;
    donation_fee_pda.config_id = config_id_addr;
    donation_fee_pda.base_mint = base_mint_addr;
    donation_fee_pda.quote_mint = WSOL_MINT;
    donation_fee_pda.creator = creator;
    donation_fee_pda.total_donated = 0;
    donation_fee_pda.last_crank_ts = 0;

    emit!(DonationFeePdaCreated {
        timestamp,
        created_by: ctx.accounts.payer.address(),
        donation_fee_pda: ctx.accounts.donation_fee_pda.address(),
        config_id: config_id_addr,
        base_mint: base_mint_addr,
        quote_mint: WSOL_MINT,
        creator,
    });

    Ok(())
}
