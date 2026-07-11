use naclac_lang::prelude::*;
use crate::components::pool_state::PoolState;
use crate::constants::SEED_POOL;

#[derive(Accounts)]
#[instruction(id: u64, pool_bump: u8, _amount_a: u64, _amount_b: u64)]
pub struct Initialize {
    #[account(mut, unsafe(alias))]
    pub payer: Signer,

    pub token_a_mint: Account<Mint>,
    pub token_b_mint: Account<Mint>,

    #[account(
        init,
        payer = payer,
        seeds = [b"pool", token_a_mint, token_b_mint, &id.to_le_bytes()],
        bump = pool_bump
    )]
    pub pool_state: Account<PoolState>,

    #[account(mut)]
    pub vault_a: Account<TokenAccount>,

    #[account(mut)]
    pub vault_b: Account<TokenAccount>,

    #[account(mut)]
    pub lp_mint: Account<Mint>,

    #[account(mut)]
    pub depositor_token_a: Account<TokenAccount>,

    #[account(mut)]
    pub depositor_token_b: Account<TokenAccount>,

    #[account(mut)]
    pub depositor_lp: Account<TokenAccount>,

    pub depositor_authority: Signer,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn initialize(ctx: Context<Initialize>, id: u64, pool_bump: u8, amount_a: u64, amount_b: u64) -> Result {
    require!(amount_a > 0 && amount_b > 0, crate::errors::AmmError::ZeroAmount);

    // Validate vaults match the mints
    require!(
        ctx.accounts.vault_a.mint() == ctx.accounts.token_a_mint.address(),
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.vault_b.mint() == ctx.accounts.token_b_mint.address(),
        crate::errors::AmmError::InvalidMint
    );
    require!(
        ctx.accounts.vault_a.owner() == ctx.accounts.pool_state.address(),
        crate::errors::AmmError::Unauthorized
    );
    require!(
        ctx.accounts.vault_b.owner() == ctx.accounts.pool_state.address(),
        crate::errors::AmmError::Unauthorized
    );

    // Initialize PoolState component
    let pool = &mut ctx.accounts.pool_state;
    pool.id = id;
    pool.token_a_mint = ctx.accounts.token_a_mint.address();
    pool.token_b_mint = ctx.accounts.token_b_mint.address();
    pool.vault_a = ctx.accounts.vault_a.address();
    pool.vault_b = ctx.accounts.vault_b.address();
    pool.lp_mint = ctx.accounts.lp_mint.address();
    pool.bump = pool_bump;

    // Transfer token A and B to the vaults
    ctx.accounts.token_program.transfer(
        TransferAccounts {
            from: &mut ctx.accounts.depositor_token_a,
            to: &mut ctx.accounts.vault_a,
            authority: &ctx.accounts.depositor_authority,
        },
        amount_a,
    )?;

    ctx.accounts.token_program.transfer(
        TransferAccounts {
            from: &mut ctx.accounts.depositor_token_b,
            to: &mut ctx.accounts.vault_b,
            authority: &ctx.accounts.depositor_authority,
        },
        amount_b,
    )?;

    // Calculate LP amount to mint using system math
    let lp_amount = crate::systems::process_initialize_lp_amount(amount_a, amount_b)?;

    // Prepare signer seeds for the pool state PDA
    let token_a_ref = ctx.accounts.token_a_mint.address();
    let token_b_ref = ctx.accounts.token_b_mint.address();
    let id_bytes = id.to_le_bytes();
    let pool_seeds: &[&[u8]] = &[
        SEED_POOL,
        token_a_ref.as_ref(),
        token_b_ref.as_ref(),
        &id_bytes,
        &[pool_bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[pool_seeds];

    // Mint LP tokens to depositor
    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.depositor_lp,
            authority: &ctx.accounts.pool_state,
        },
        lp_amount,
        signer_seeds,
    )?;

    emit!(crate::events::PoolInitialized {
        token_a_mint: ctx.accounts.token_a_mint.address(),
        token_b_mint: ctx.accounts.token_b_mint.address(),
        lp_amount,
    });

    Ok(())
}
