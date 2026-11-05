//! AMM remove liquidity instruction.

use {
    crate::{
        constants::SEED_POOL, events::LiquidityRemoved,
        state::pool_state::PoolState, systems::process_remove_liquidity_math,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct RemoveLiquidity {
    #[account(mut)]
    pub user: Signer,

    #[account(
        mut,
        token_a_mint = token_a_mint,
        token_b_mint = token_b_mint,
        lp_mint = lp_mint,
        vault_a = vault_a,
        vault_b = vault_b,
    )]
    pub pool_state: Account<PoolState>,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = pool_state,
    )]
    pub vault_a: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = pool_state,
    )]
    pub vault_b: Account<TokenAccount>,

    #[account(
        mut,
        mint::authority = pool_state,
    )]
    pub lp_mint: Account<Mint>,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = user,
    )]
    pub user_token_a: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = user,
    )]
    pub user_token_b: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = user,
    )]
    pub user_lp: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

/// Removes liquidity by burning user's LP tokens and returning proportional pool assets.
pub fn remove_liquidity(
    ctx: Context<RemoveLiquidity>,
    lp_amount: u64,
) -> Result {
    let x = ctx.accounts.vault_a.amount();
    let y = ctx.accounts.vault_b.amount();
    let lp_supply = ctx.accounts.lp_mint.supply();

    let (amount_a, amount_b) = process_remove_liquidity_math(
        x,
        y,
        lp_supply,
        lp_amount,
    )?;

    ctx.accounts.token_program.burn(
        BurnAccounts {
            mint: &mut ctx.accounts.lp_mint,
            from: &mut ctx.accounts.user_lp,
            authority: &ctx.accounts.user,
        },
        lp_amount,
    )?;

    let token_a_ref = ctx.accounts.pool_state.token_a_mint;
    let token_b_ref = ctx.accounts.pool_state.token_b_mint;
    let id_bytes = ctx.accounts.pool_state.id.to_le_bytes();
    let pool_seeds: &[&[u8]] = &[
        SEED_POOL,
        token_a_ref.as_ref(),
        token_b_ref.as_ref(),
        &id_bytes,
        &[ctx.accounts.pool_state.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[pool_seeds];

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    let token_b_decimals = ctx.accounts.token_b_mint.decimals();

    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.vault_a,
            mint: &ctx.accounts.token_a_mint,
            to: &mut ctx.accounts.user_token_a,
            authority: &ctx.accounts.pool_state,
        },
        amount_a,
        token_a_decimals,
        signer_seeds,
    )?;

    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.vault_b,
            mint: &ctx.accounts.token_b_mint,
            to: &mut ctx.accounts.user_token_b,
            authority: &ctx.accounts.pool_state,
        },
        amount_b,
        token_b_decimals,
        signer_seeds,
    )?;

    emit!(LiquidityRemoved {
        token_a_mint: ctx.accounts.pool_state.token_a_mint,
        token_b_mint: ctx.accounts.pool_state.token_b_mint,
        amount_a,
        amount_b,
        lp_burned: lp_amount,
    });

    Ok(())
}
