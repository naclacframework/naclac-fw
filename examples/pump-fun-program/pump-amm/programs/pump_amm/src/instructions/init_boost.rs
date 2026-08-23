use naclac_lang::prelude::*;
use crate::components::{GlobalConfig, Pool};
use crate::constants::{BOOST_BASE_SUPPLY_DIVISOR, BOOST_VAULT_SEED, GLOBAL_CONFIG_SEED, POOL_SEED};
use crate::errors::PumpAmmError;
use crate::events::InitBoostEvent;

// Real `init_boost` takes zero args and derives `boost_vault_authority`/
// `boost_vault` on-chain via a search loop; naclac forbids that for dynamic
// (non-compile-time-literal) seeds, so both bumps are caller-supplied here
// instead — the same pattern already used throughout this framework for any
// PDA whose seeds include a runtime account rather than only literals.
#[derive(Accounts)]
#[instruction(boost_vault_authority_bump: u8, boost_vault_bump: u8)]
pub struct InitBoost {
    /// SAFETY: only used as `InitBoostEvent`'s `bonding_curve` field — the
    /// real program derives this on-chain via `find_program_address` purely
    /// for event logging (not an account it ever reads), which naclac bans
    /// outright; our own `migrate_v2` (the only real caller) already has
    /// this account in its own context and passes it straight through.
    /// Never deserialized.
    pub bonding_curve: AccountInfo,

    #[account(
        mut,
        base_mint = base_mint,
        quote_mint = quote_mint,
        pool_base_token_account = pool_base_token_account,
        pool_quote_token_account = pool_quote_token_account,
    )]
    pub pool: Account<Pool>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,

    #[account(mut)]
    pub creator: Signer,

    pub base_mint: InterfaceAccount<Mint>,
    pub quote_mint: InterfaceAccount<Mint>,

    pub pool_base_token_account: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub pool_quote_token_account: InterfaceAccount<TokenAccount>,

    /// SAFETY: `seeds`/`bump` already verifies its address; a bare
    /// signing/seed PDA with no stored data (matches the real program — no
    /// dedicated `BoostVault`-type account exists), used below only as a CPI
    /// signer and as `boost_vault`'s ATA authority.
    #[account(seeds = [BOOST_VAULT_SEED, pool.address().as_ref()], bump = boost_vault_authority_bump)]
    pub boost_vault_authority: AccountInfo,

    /// SAFETY: `init` + `associated_token::mint`/`::authority`/`::bump`
    /// below fully validate and construct this account via a real CPI —
    /// there is no naclac `Discriminator` to check since this is a raw SPL
    /// `TokenAccount` layout (same reasoning as `create_pool`'s own
    /// freshly-`init`ed ATA fields).
    #[account(
        init,
        payer = creator,
        associated_token::mint = quote_mint,
        associated_token::authority = boost_vault_authority,
        associated_token::bump = boost_vault_bump,
        token::program = quote_token_program,
    )]
    pub boost_vault: AccountInfo,

    pub quote_token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,
    pub associated_token_program: Program<AssociatedToken>,
}

/// Real behavior confirmed via `probe46.rs`/`probe48.rs`/`probe49.rs`
/// (litesvm, real bytecode) and 5 independent real mainnet `migrate_v2`
/// transactions (`docs/plan/amm-03-boost-mechanism.md`): `init_boost` moves
/// no funds of its own accord and computes no percentage — it reads
/// `pool_quote_token_account`'s and `pool_base_token_account`'s balances
/// *as they stand at call time*, computes `virtual_quote_reserves =
/// floor(quote_balance * base_balance / BOOST_BASE_SUPPLY_DIVISOR)`,
/// transfers that amount out of `pool_quote_token_account` into a
/// freshly-created `boost_vault`, and records it on `pool`. Confirmed with
/// zero error across 5 independent real transactions plus a direct blind
/// litesvm replication using a 9-decimal quote mint (ruling out a
/// quote-decimals dependency).
#[instruction]
pub fn init_boost(
    ctx: Context<InitBoost>,
    _boost_vault_authority_bump: u8,
    _boost_vault_bump: u8,
) -> Result {
    require!(bool::from(ctx.accounts.global_config.boost_enabled), PumpAmmError::BoostDisabled);

    let quote_balance = ctx.accounts.pool_quote_token_account.amount();
    let base_balance = ctx.accounts.pool_base_token_account.amount();
    let virtual_quote_reserves = (quote_balance as u128)
        .checked_mul(base_balance as u128)
        .and_then(|v| v.checked_div(BOOST_BASE_SUPPLY_DIVISOR))
        .ok_or(PumpAmmError::MathOverflow)?;
    let virtual_quote_reserves_u64 =
        u64::try_from(virtual_quote_reserves).map_err(|_| PumpAmmError::MathOverflow)?;

    let pool_address = ctx.accounts.pool.address();
    let pool_bump = ctx.accounts.pool.pool_bump;
    let pool_index_bytes = ctx.accounts.pool.index.to_le_bytes();
    let pool_creator = ctx.accounts.pool.creator;
    let pool_base_mint = ctx.accounts.pool.base_mint;
    let pool_quote_mint = ctx.accounts.pool.quote_mint;
    let pool_signer_seeds: &[&[u8]] = &[
        POOL_SEED,
        &pool_index_bytes,
        pool_creator.as_ref(),
        pool_base_mint.as_ref(),
        pool_quote_mint.as_ref(),
        &[pool_bump],
    ];
    let pool_signer: &[&[&[u8]]] = &[pool_signer_seeds];

    ctx.accounts.quote_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.pool_quote_token_account,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.boost_vault,
            authority: &ctx.accounts.pool,
        },
        virtual_quote_reserves_u64,
        ctx.accounts.quote_mint.decimals(),
        pool_signer,
    )?;

    let real_quote_reserves_after = quote_balance - virtual_quote_reserves_u64;
    ctx.accounts.pool.virtual_quote_reserves = virtual_quote_reserves.to_le_bytes();

    let timestamp = unix_timestamp()?;
    emit!(InitBoostEvent {
        timestamp,
        virtual_quote_reserves: virtual_quote_reserves as i128,
        mint: pool_base_mint,
        bonding_curve: ctx.accounts.bonding_curve.address(),
        pool: pool_address,
        real_quote_reserves_after,
    });

    Ok(())
}
