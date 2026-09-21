use naclac_lang::prelude::*;
use crate::components::pool_state::PoolState;
use crate::constants::SEED_POOL;

#[derive(Accounts)]
pub struct Swap {
    #[account(mut)]
    pub user: Signer,

    pub pool_state: Account<PoolState>,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(mut)]
    pub pool_source_vault: Account<TokenAccount>,

    #[account(mut)]
    pub pool_destination_vault: Account<TokenAccount>,

    #[account(mut)]
    pub user_source_token: Account<TokenAccount>,

    #[account(mut)]
    pub user_destination_token: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

pub fn swap(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result {
    require!(amount_in > 0, crate::errors::AmmError::ZeroAmount);

    // Validate that the vaults belong to this pool
    require!(
        (ctx.accounts.pool_source_vault.address() == ctx.accounts.pool_state.vault_a &&
         ctx.accounts.pool_destination_vault.address() == ctx.accounts.pool_state.vault_b) ||
        (ctx.accounts.pool_source_vault.address() == ctx.accounts.pool_state.vault_b &&
         ctx.accounts.pool_destination_vault.address() == ctx.accounts.pool_state.vault_a),
        crate::errors::AmmError::InvalidMint
    );

    // Validate ownership/mint details
    require!(
        ctx.accounts.user_source_token.mint() == ctx.accounts.pool_source_vault.mint(),
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.user_destination_token.mint() == ctx.accounts.pool_destination_vault.mint(),
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.token_a_mint.address() == ctx.accounts.pool_state.token_a_mint,
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.token_b_mint.address() == ctx.accounts.pool_state.token_b_mint,
        crate::errors::AmmError::InvalidMint
    );

    let x = ctx.accounts.pool_source_vault.amount();
    let y = ctx.accounts.pool_destination_vault.amount();

    // Call swap math system function
    let amount_out = crate::systems::process_swap_output(x, y, amount_in)?;

    require!(amount_out >= minimum_amount_out, crate::errors::AmmError::SlippageExceeded);

    let source_is_token_a = ctx.accounts.pool_source_vault.mint() == ctx.accounts.token_a_mint.address();
    let source_mint = if source_is_token_a { &ctx.accounts.token_a_mint } else { &ctx.accounts.token_b_mint };
    let destination_mint = if source_is_token_a { &ctx.accounts.token_b_mint } else { &ctx.accounts.token_a_mint };
    let source_decimals = source_mint.decimals();
    let destination_decimals = destination_mint.decimals();

    // 1. Transfer input tokens from user to the pool's source vault
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

    // 2. Prepare signer seeds for the pool state PDA (including ID)
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

    // 3. Transfer output tokens from the pool's destination vault to the user
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

    emit!(crate::events::SwapExecuted {
        user: ctx.accounts.user.address(),
        amount_in,
        amount_out,
    });

    Ok(())
}
