use naclac_lang::prelude::*;
use crate::components::{GlobalConfig, Pool};
use crate::constants::{BOOST_VAULT_SEED, GLOBAL_CONFIG_SEED, POOL_SEED};
use crate::errors::PumpAmmError;
use crate::events::BoostBuyAndBurnEvent;

// `boost_vault_authority_bump` is naclac-only (see `init_boost.rs`'s own
// comment on the same pattern) — the real instruction takes only
// `quote_amount_in`/`min_base_amount_burned`.
#[derive(Accounts)]
#[instruction(quote_amount_in: u64, min_base_amount_burned: u64, boost_vault_authority_bump: u8)]
pub struct BoostBuyAndBurn {
    /// SAFETY: only used as `BoostBuyAndBurnEvent`'s `bonding_curve` field
    /// (see `init_boost.rs`'s own account of the same name and reasoning —
    /// the real program derives this on-chain purely for event logging,
    /// which naclac bans; the caller computes it off-chain and passes it
    /// in). Never deserialized.
    pub bonding_curve: AccountInfo,

    #[account(
        mut,
        base_mint = base_mint,
        quote_mint = quote_mint,
        pool_base_token_account = pool_base_token_account,
        pool_quote_token_account = pool_quote_token_account,
    )]
    pub pool: Account<Pool>,

    #[account(mut)]
    pub authority: Signer,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,

    #[account(mut)]
    pub base_mint: InterfaceAccount<Mint>,
    pub quote_mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub pool_base_token_account: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub pool_quote_token_account: InterfaceAccount<TokenAccount>,

    /// SAFETY: `seeds`/`bump` already verifies its address; a bare
    /// signing/seed PDA with no stored data, used below only as a CPI
    /// signer (matches `init_boost.rs`'s own account of the same name).
    #[account(seeds = [BOOST_VAULT_SEED, pool.address().as_ref()], bump = boost_vault_authority_bump)]
    pub boost_vault_authority: AccountInfo,

    #[account(mut)]
    pub boost_vault: InterfaceAccount<TokenAccount>,

    pub base_token_program: Interface<TokenInterface>,
    pub quote_token_program: Interface<TokenInterface>,
}

/// Real formula confirmed via `probe46.rs` (litesvm, real bytecode),
/// `docs/plan/amm-03-boost-mechanism.md`: internally a genuine constant-
/// product buy against the pool using `effective_quote_reserves = real +
/// virtual` as the pricing denominator, with the same "amount − 1"
/// conservative-rounding pattern already confirmed for `buy_exact_sol_in`
/// in the bonding-curve program: `base_out = floor(base_reserves *
/// (net_quote - 1) / (effective_quote_reserves + net_quote - 1))`. 100% of
/// `base_out` is burned (no partial-burn leftover observed).
/// `pool.virtual_quote_reserves` is left completely unchanged — it's a
/// fixed pricing-depth constant set once by `init_boost`, not a running
/// counter.
///
/// `quote_amount_in_used` is capped at `boost_vault`'s current balance —
/// this specific capping behavior (vs. e.g. erroring outright) was not
/// empirically exercised (the one real/probe example available always had
/// `quote_amount_in_used == quote_amount_in_requested`), inferred from the
/// event's separate `_requested`/`_used` fields, which would otherwise have
/// no reason to differ.
#[instruction]
pub fn boost_buy_and_burn(
    ctx: Context<BoostBuyAndBurn>,
    quote_amount_in: u64,
    min_base_amount_burned: u64,
    _boost_vault_authority_bump: u8,
) -> Result {
    require!(bool::from(ctx.accounts.global_config.boost_enabled), PumpAmmError::BoostDisabled);
    require!(ctx.accounts.pool.virtual_quote_reserves != [0u8; 16], PumpAmmError::PoolCannotBoost);
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global_config.boost_authority,
        PumpAmmError::InvalidAdmin
    );

    let boost_vault_balance = ctx.accounts.boost_vault.amount();
    let quote_amount_in_used = quote_amount_in.min(boost_vault_balance);
    require!(quote_amount_in_used > 0, PumpAmmError::MathOverflow);

    let real_quote_reserves = ctx.accounts.pool_quote_token_account.amount();
    let virtual_quote_reserves = i128::from_le_bytes(ctx.accounts.pool.virtual_quote_reserves);
    let effective_quote_reserves = (real_quote_reserves as u128)
        .checked_add(virtual_quote_reserves as u128)
        .ok_or(PumpAmmError::MathOverflow)?;
    let base_reserves = ctx.accounts.pool_base_token_account.amount();

    let net_quote = (quote_amount_in_used as u128).checked_sub(1).ok_or(PumpAmmError::MathOverflow)?;
    let denominator = effective_quote_reserves.checked_add(net_quote).ok_or(PumpAmmError::MathOverflow)?;
    let base_out_u128 = (base_reserves as u128)
        .checked_mul(net_quote)
        .and_then(|v| v.checked_div(denominator))
        .ok_or(PumpAmmError::MathOverflow)?;
    let base_out = u64::try_from(base_out_u128).map_err(|_| PumpAmmError::MathOverflow)?;
    require!(base_out >= min_base_amount_burned, PumpAmmError::TooLittlePoolTokenLiquidity);

    let pool_address = ctx.accounts.pool.address();
    let boost_vault_authority_seeds: &[&[u8]] =
        &[BOOST_VAULT_SEED, pool_address.as_ref(), &[_boost_vault_authority_bump]];
    let boost_vault_authority_signer: &[&[&[u8]]] = &[boost_vault_authority_seeds];

    ctx.accounts.quote_token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.boost_vault,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.pool_quote_token_account,
            authority: &ctx.accounts.boost_vault_authority,
        },
        quote_amount_in_used,
        ctx.accounts.quote_mint.decimals(),
        boost_vault_authority_signer,
    )?;

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

    ctx.accounts.base_token_program.burn_signed(
        BurnAccounts {
            mint: &mut ctx.accounts.base_mint,
            from: &mut ctx.accounts.pool_base_token_account,
            authority: &ctx.accounts.pool,
        },
        base_out,
        pool_signer,
    )?;

    let real_quote_reserves_after = real_quote_reserves + quote_amount_in_used;
    let base_reserves_after = base_reserves - base_out;
    let boost_vault_remaining = boost_vault_balance - quote_amount_in_used;

    let timestamp = unix_timestamp()?;
    emit!(BoostBuyAndBurnEvent {
        timestamp,
        virtual_quote_reserves,
        mint: pool_base_mint,
        bonding_curve: ctx.accounts.bonding_curve.address(),
        pool: pool_address,
        authority: ctx.accounts.authority.address(),
        quote_amount_in_requested: quote_amount_in,
        quote_amount_in_used,
        base_amount_burned: base_out,
        real_quote_reserves_after,
        base_reserves_after,
        boost_vault_remaining,
    });

    Ok(())
}
