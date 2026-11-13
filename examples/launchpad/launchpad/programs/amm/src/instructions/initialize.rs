//! AMM pool initialization instruction.

use {
    crate::{
        constants::{LP_DECIMALS, SEED_LP_MINT, SEED_VAULT},
        errors::AmmError,
        events::PoolInitialized,
        state::pool_state::PoolState,
        systems::process_initialize_lp_amount,
        validation::{is_ascending, require_supported_mint},
    },
    naclac_lang::prelude::*,
};

/// `payer` funds every account the pool creates and is also the depositor of
/// the pool's first liquidity. The program creates both vaults, the LP mint and
/// the depositor's LP token account itself, so none of them can arrive
/// pre-configured by the caller. Each mint must be owned by its own token
/// program account.
#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    #[account(mint::token_program = token_program_a)]
    pub token_a_mint: InterfaceAccount<Mint>,

    #[account(mint::token_program = token_program_b)]
    pub token_b_mint: InterfaceAccount<Mint>,

    #[account(
        init,
        payer = payer,
        seeds = PoolState::seeds(token_a_mint.address(), token_b_mint.address(), id),
        bump
    )]
    pub pool_state: Account<PoolState>,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below construct and
    /// validate this account through a CPI to the classic Token program; it is
    /// a raw SPL `Mint` with no naclac discriminator to check.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_LP_MINT, pool_state.address().as_ref()],
        bump,
        mint::decimals = LP_DECIMALS,
        mint::authority = pool_state,
        mint::token_program = lp_token_program,
    )]
    pub lp_mint: AccountInfo,

    /// SAFETY: `init` + `token::*` below create this account and initialise
    /// it through the named token program, at an address only this program can
    /// sign for; it is a raw SPL token account with no naclac discriminator to
    /// check.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT, pool_state.address().as_ref(), token_a_mint.address().as_ref()],
        bump,
        token::mint = token_a_mint,
        token::authority = pool_state,
        token::token_program = token_program_a,
    )]
    pub vault_a: AccountInfo,

    /// SAFETY: same as `vault_a`.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_VAULT, pool_state.address().as_ref(), token_b_mint.address().as_ref()],
        bump,
        token::mint = token_b_mint,
        token::authority = pool_state,
        token::token_program = token_program_b,
    )]
    pub vault_b: AccountInfo,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = payer,
    )]
    pub depositor_token_a: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = payer,
    )]
    pub depositor_token_b: InterfaceAccount<TokenAccount>,

    /// SAFETY: `init` + `associated_token::*` below have the Associated Token
    /// program create and address-check this account; it is a raw SPL token
    /// account with no naclac discriminator to check. The LP mint is created
    /// in this same instruction, so nobody can create this account first.
    #[account(
        init,
        payer = payer,
        associated_token::mint = lp_mint,
        associated_token::authority = payer,
        associated_token::token_program = lp_token_program,
    )]
    pub depositor_lp: AccountInfo,

    pub token_program_a: Interface<TokenInterface>,

    /// SAFETY: repeats `token_program_a` when both mints use one token program.
    #[account(dup)]
    pub token_program_b: Interface<TokenInterface>,

    /// SAFETY: repeats a pool mint's token program when that mint is classic Token.
    #[account(dup)]
    pub lp_token_program: Program<Token>,

    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

pub fn initialize(ctx: Context<Initialize>, id: u64, amount_a: u64, amount_b: u64) -> Result {
    require!(amount_a > 0 && amount_b > 0, AmmError::ZeroAmount);
    require!(
        is_ascending(
            &ctx.accounts.token_a_mint.address(),
            &ctx.accounts.token_b_mint.address()
        ),
        AmmError::InvalidMint
    );
    require_supported_mint(&ctx.accounts.token_a_mint)?;
    require_supported_mint(&ctx.accounts.token_b_mint)?;

    let pool = &mut ctx.accounts.pool_state;
    pool.id = id;
    pool.token_a_mint = ctx.accounts.token_a_mint.address();
    pool.token_b_mint = ctx.accounts.token_b_mint.address();
    pool.vault_a = ctx.accounts.vault_a.address();
    pool.vault_b = ctx.accounts.vault_b.address();
    pool.lp_mint = ctx.accounts.lp_mint.address();
    pool.bump = ctx.bumps.pool_state;

    let token_a_decimals = ctx.accounts.token_a_mint.decimals();
    ctx.accounts.token_program_a.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.depositor_token_a,
            mint: &ctx.accounts.token_a_mint,
            to: &mut ctx.accounts.vault_a,
            authority: &ctx.accounts.payer,
        },
        amount_a,
        token_a_decimals,
    )?;

    let token_b_decimals = ctx.accounts.token_b_mint.decimals();
    ctx.accounts.token_program_b.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.depositor_token_b,
            mint: &ctx.accounts.token_b_mint,
            to: &mut ctx.accounts.vault_b,
            authority: &ctx.accounts.payer,
        },
        amount_b,
        token_b_decimals,
    )?;

    let lp_amount = process_initialize_lp_amount(amount_a, amount_b)?;

    let token_a_address = ctx.accounts.token_a_mint.address();
    let token_b_address = ctx.accounts.token_b_mint.address();
    let pool_seed_set = PoolState::seeds(&token_a_address, &token_b_address, &id);
    let pool_bump = [ctx.bumps.pool_state];
    let pool_seeds = pool_seed_set.signer_seeds(&pool_bump);

    ctx.accounts.lp_token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.depositor_lp,
            authority: &ctx.accounts.pool_state,
        },
        lp_amount,
        &[pool_seeds.signer()],
    )?;

    emit!(PoolInitialized {
        token_a_mint: ctx.accounts.token_a_mint.address(),
        token_b_mint: ctx.accounts.token_b_mint.address(),
        lp_amount,
    });

    Ok(())
}
