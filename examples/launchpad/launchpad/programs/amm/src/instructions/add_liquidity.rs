//! AMM add liquidity instruction.

use {
    crate::{
        constants::SEED_POOL,
        errors::AmmError,
        events::LiquidityAdded,
        state::pool_state::PoolState,
        systems::process_add_liquidity_math,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct AddLiquidity {
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

pub fn add_liquidity(
    ctx: Context<AddLiquidity>,
    max_amount_a: u64,
    max_amount_b: u64,
    min_lp: u64,
) -> Result {
    let x = ctx.accounts.vault_a.amount();
    let y = ctx.accounts.vault_b.amount();
    let lp_supply = ctx.accounts.lp_mint.supply();

    let (deposit_a, deposit_b, lp_to_mint) = process_add_liquidity_math(
        x,
        y,
        lp_supply,
        max_amount_a,
        max_amount_b,
    )?;
    require!(lp_to_mint >= min_lp, AmmError::SlippageExceeded);

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    let token_b_decimals = ctx.accounts.token_b_mint.decimals();

    ctx.accounts.token_program_a.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_a,
            mint: &ctx.accounts.token_a_mint,
            to: &mut ctx.accounts.vault_a,
            authority: &ctx.accounts.user,
        },
        deposit_a,
        token_a_decimals,
    )?;

    ctx.accounts.token_program_b.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_b,
            mint: &ctx.accounts.token_b_mint,
            to: &mut ctx.accounts.vault_b,
            authority: &ctx.accounts.user,
        },
        deposit_b,
        token_b_decimals,
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

    ctx.accounts.lp_token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.user_lp,
            authority: &ctx.accounts.pool_state,
        },
        lp_to_mint,
        signer_seeds,
    )?;

    emit!(LiquidityAdded {
        token_a_mint: ctx.accounts.pool_state.token_a_mint,
        token_b_mint: ctx.accounts.pool_state.token_b_mint,
        amount_a: deposit_a,
        amount_b: deposit_b,
        lp_minted: lp_to_mint,
    });

    Ok(())
}
