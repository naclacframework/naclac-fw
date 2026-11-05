//! AMM token swap instruction.

use {
    crate::{
        constants::SEED_POOL, errors::AmmError, events::SwapExecuted,
        state::pool_state::PoolState, systems::process_swap_output,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Swap {
    #[account(mut)]
    pub user: Signer,

    #[account(
        token_a_mint = token_a_mint,
        token_b_mint = token_b_mint,
    )]
    pub pool_state: Account<PoolState>,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(
        mut,
        token::authority = pool_state,
    )]
    pub pool_source_vault: Account<TokenAccount>,

    #[account(
        mut,
        token::authority = pool_state,
    )]
    pub pool_destination_vault: Account<TokenAccount>,

    #[account(
        mut,
        token::authority = user,
    )]
    pub user_source_token: Account<TokenAccount>,

    #[account(
        mut,
        token::authority = user,
    )]
    pub user_destination_token: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

/// Swaps an exact input amount of one pool token for output tokens of the other.
pub fn swap(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result {
    require!(amount_in > 0, AmmError::ZeroAmount);

    require!(
        (ctx.accounts.pool_source_vault.address() == ctx.accounts.pool_state.vault_a
            && ctx.accounts.pool_destination_vault.address() == ctx.accounts.pool_state.vault_b)
            || (ctx.accounts.pool_source_vault.address() == ctx.accounts.pool_state.vault_b
                && ctx.accounts.pool_destination_vault.address() == ctx.accounts.pool_state.vault_a),
        AmmError::InvalidMint
    );

    require!(
        ctx.accounts.user_source_token.mint() == ctx.accounts.pool_source_vault.mint(),
        AmmError::InvalidMint
    );
    require!(
        ctx.accounts.user_destination_token.mint() == ctx.accounts.pool_destination_vault.mint(),
        AmmError::InvalidMint
    );

    let x = ctx.accounts.pool_source_vault.amount();
    let y = ctx.accounts.pool_destination_vault.amount();

    let amount_out = process_swap_output(x, y, amount_in)?;
    require!(amount_out >= minimum_amount_out, AmmError::SlippageExceeded);

    let source_is_token_a = ctx.accounts.pool_source_vault.mint() == ctx.accounts.token_a_mint.address();
    let source_mint = if source_is_token_a { &ctx.accounts.token_a_mint } else { &ctx.accounts.token_b_mint };
    let destination_mint = if source_is_token_a { &ctx.accounts.token_b_mint } else { &ctx.accounts.token_a_mint };
    let source_decimals = source_mint.decimals();
    let destination_decimals = destination_mint.decimals();

    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_source_token,
            mint: source_mint,
            to: &mut ctx.accounts.pool_source_vault,
            authority: &ctx.accounts.user,
        },
        amount_in,
        source_decimals,
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

    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.pool_destination_vault,
            mint: destination_mint,
            to: &mut ctx.accounts.user_destination_token,
            authority: &ctx.accounts.pool_state,
        },
        amount_out,
        destination_decimals,
        signer_seeds,
    )?;

    emit!(SwapExecuted {
        user: ctx.accounts.user.address(),
        amount_in,
        amount_out,
    });

    Ok(())
}
