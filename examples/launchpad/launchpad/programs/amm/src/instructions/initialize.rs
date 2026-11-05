//! AMM pool initialization instruction.

use {
    crate::{
        constants::SEED_POOL, errors::AmmError, events::PoolInitialized,
        state::pool_state::PoolState, systems::process_initialize_lp_amount,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_POOL, token_a_mint.address().as_ref(), token_b_mint.address().as_ref(), &id.to_le_bytes()],
        bump
    )]
    pub pool_state: Account<PoolState>,

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
        token::authority = depositor_authority,
    )]
    pub depositor_token_a: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = depositor_authority,
    )]
    pub depositor_token_b: Account<TokenAccount>,

    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = depositor_authority,
    )]
    pub depositor_lp: Account<TokenAccount>,

    pub depositor_authority: Signer,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

/// Initializes an AMM pool with initial liquidity and mints corresponding LP tokens to the depositor.
pub fn initialize(ctx: Context<Initialize>, id: u64, amount_a: u64, amount_b: u64) -> Result {
    require!(amount_a > 0 && amount_b > 0, AmmError::ZeroAmount);

    let pool = &mut ctx.accounts.pool_state;
    pool.id = id;
    pool.token_a_mint = ctx.accounts.token_a_mint.address();
    pool.token_b_mint = ctx.accounts.token_b_mint.address();
    pool.vault_a = ctx.accounts.vault_a.address();
    pool.vault_b = ctx.accounts.vault_b.address();
    pool.lp_mint = ctx.accounts.lp_mint.address();
    pool.bump = ctx.bumps.pool_state;

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.depositor_token_a,
            mint: &ctx.accounts.token_a_mint,
            to: &mut ctx.accounts.vault_a,
            authority: &ctx.accounts.depositor_authority,
        },
        amount_a,
        token_a_decimals,
    )?;

    let token_b_decimals = ctx.accounts.token_b_mint.decimals();
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.depositor_token_b,
            mint: &ctx.accounts.token_b_mint,
            to: &mut ctx.accounts.vault_b,
            authority: &ctx.accounts.depositor_authority,
        },
        amount_b,
        token_b_decimals,
    )?;

    let lp_amount = process_initialize_lp_amount(amount_a, amount_b)?;

    let token_a_ref = ctx.accounts.token_a_mint.address();
    let token_b_ref = ctx.accounts.token_b_mint.address();
    let id_bytes = id.to_le_bytes();
    let pool_bump = ctx.bumps.pool_state;
    let pool_seeds: &[&[u8]] = &[
        SEED_POOL,
        token_a_ref.as_ref(),
        token_b_ref.as_ref(),
        &id_bytes,
        &[pool_bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[pool_seeds];

    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.depositor_lp,
            authority: &ctx.accounts.pool_state,
        },
        lp_amount,
        signer_seeds,
    )?;

    emit!(PoolInitialized {
        token_a_mint: ctx.accounts.token_a_mint.address(),
        token_b_mint: ctx.accounts.token_b_mint.address(),
        lp_amount,
    });

    Ok(())
}
