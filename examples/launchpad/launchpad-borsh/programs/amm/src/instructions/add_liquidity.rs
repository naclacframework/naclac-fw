use naclac_lang::prelude::*;
use crate::components::pool_state::PoolState;
use crate::constants::SEED_POOL;

#[derive(Accounts)]
pub struct AddLiquidity {
    #[account(mut)]
    pub user: Signer,

    #[account(mut)]
    pub pool_state: Account<PoolState>,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(mut)]
    pub vault_a: Account<TokenAccount>,

    #[account(mut)]
    pub vault_b: Account<TokenAccount>,

    #[account(mut)]
    pub lp_mint: Account<Mint>,

    #[account(mut)]
    pub user_token_a: Account<TokenAccount>,

    #[account(mut)]
    pub user_token_b: Account<TokenAccount>,

    #[account(mut)]
    pub user_lp: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

pub fn add_liquidity(
    ctx: Context<AddLiquidity>,
    max_amount_a: u64,
    max_amount_b: u64,
) -> Result {
    require!(
        ctx.accounts.token_a_mint.address() == ctx.accounts.pool_state.token_a_mint,
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.token_b_mint.address() == ctx.accounts.pool_state.token_b_mint,
        crate::errors::AmmError::InvalidMint
    );

    let x = ctx.accounts.vault_a.amount();
    let y = ctx.accounts.vault_b.amount();
    let lp_supply = ctx.accounts.lp_mint.supply();

    // Call add liquidity math system function
    let (deposit_a, deposit_b, lp_to_mint) = crate::systems::process_add_liquidity_math(
        x,
        y,
        lp_supply,
        max_amount_a,
        max_amount_b,
    )?;

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    let token_b_decimals = ctx.accounts.token_b_mint.decimals();

    // 1. Transfer Token A and B from user to pool vaults
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_a,
            mint: &ctx.accounts.token_a_mint,
            to: &mut ctx.accounts.vault_a,
            authority: &ctx.accounts.user,
        },
        deposit_a,
        token_a_decimals,
    )?;

    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_b,
            mint: &ctx.accounts.token_b_mint,
            to: &mut ctx.accounts.vault_b,
            authority: &ctx.accounts.user,
        },
        deposit_b,
        token_b_decimals,
    )?;

    // 2. Prepare signer seeds for the pool state PDA
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

    // 3. Mint LP tokens to the user
    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.user_lp,
            authority: &ctx.accounts.pool_state,
        },
        lp_to_mint,
        signer_seeds,
    )?;

    emit!(crate::events::LiquidityAdded {
        token_a_mint: ctx.accounts.pool_state.token_a_mint,
        token_b_mint: ctx.accounts.pool_state.token_b_mint,
        amount_a: deposit_a,
        amount_b: deposit_b,
        lp_minted: lp_to_mint,
    });

    Ok(())
}
