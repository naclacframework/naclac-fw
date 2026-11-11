//! AMM remove liquidity instruction.

use {
    crate::{
        errors::AmmError,
        events::LiquidityRemoved,
        state::pool_state::PoolState,
        systems::process_remove_liquidity_math,
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

    #[account(mint::token_program = token_program_a)]
    pub token_a_mint: InterfaceAccount<Mint>,

    #[account(mint::token_program = token_program_b)]
    pub token_b_mint: InterfaceAccount<Mint>,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = pool_state,
    )]
    pub vault_a: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = pool_state,
    )]
    pub vault_b: InterfaceAccount<TokenAccount>,

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
    pub user_token_a: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = user,
    )]
    pub user_token_b: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = user,
    )]
    pub user_lp: Account<TokenAccount>,

    pub token_program_a: Interface<TokenInterface>,

    /// SAFETY: repeats `token_program_a` when both mints use one token program.
    #[account(dup)]
    pub token_program_b: Interface<TokenInterface>,

    /// SAFETY: repeats a pool mint's token program when that mint is classic Token.
    #[account(dup)]
    pub lp_token_program: Program<Token>,
}

pub fn remove_liquidity(
    ctx: Context<RemoveLiquidity>,
    lp_amount: u64,
    min_amount_a: u64,
    min_amount_b: u64,
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
    require!(
        amount_a >= min_amount_a && amount_b >= min_amount_b,
        AmmError::SlippageExceeded
    );

    ctx.accounts.lp_token_program.burn(
        BurnAccounts {
            mint: &mut ctx.accounts.lp_mint,
            from: &mut ctx.accounts.user_lp,
            authority: &ctx.accounts.user,
        },
        lp_amount,
    )?;

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    let token_b_decimals = ctx.accounts.token_b_mint.decimals();

    ctx.accounts.pool_state.with_signer_seeds(|signer_seeds| {
        ctx.accounts.token_program_a.transfer_checked_signed(
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

        ctx.accounts.token_program_b.transfer_checked_signed(
            TransferCheckedAccounts {
                from: &mut ctx.accounts.vault_b,
                mint: &ctx.accounts.token_b_mint,
                to: &mut ctx.accounts.user_token_b,
                authority: &ctx.accounts.pool_state,
            },
            amount_b,
            token_b_decimals,
            signer_seeds,
        )
    })?;

    emit!(LiquidityRemoved {
        token_a_mint: ctx.accounts.pool_state.token_a_mint,
        token_b_mint: ctx.accounts.pool_state.token_b_mint,
        amount_a,
        amount_b,
        lp_burned: lp_amount,
    });

    Ok(())
}
