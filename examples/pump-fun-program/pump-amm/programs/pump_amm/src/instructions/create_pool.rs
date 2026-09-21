use naclac_lang::prelude::*;
use crate::components::{GlobalConfig, Pool};
use crate::constants::{
    DISABLE_CREATE_POOL_FLAG, GLOBAL_CONFIG_SEED, LP_BOOTSTRAP_WITHHELD, LP_MINT_DECIMALS,
    POOL_LP_MINT_SEED, POOL_SEED,
};
use crate::errors::PumpAmmError;
use crate::events::CreatePoolEvent;
use crate::systems::lp_bootstrap_amount;

// Real `create_pool` derives every PDA bump on-chain via a search loop;
// naclac forbids that for dynamic (non-compile-time-literal) seeds, so every
// PDA below takes an explicit caller-supplied bump instead — the same
// pattern already used throughout `pump`/`pump_fees` for any PDA whose seeds
// include a runtime account/arg rather than only literals.
#[instruction_args]
pub struct CreatePoolArgs {
    pub index: u16,
    pub base_amount_in: u64,
    pub quote_amount_in: u64,
    pub coin_creator: Address,
    pub is_mayhem_mode: Bool,
    pub is_cashback_coin: Bool,
    pub pool_bump: u8,
    pub lp_mint_bump: u8,
    pub user_base_token_account_bump: u8,
    pub user_quote_token_account_bump: u8,
}

#[derive(Accounts)]
#[instruction(args: CreatePoolArgs)]
pub struct CreatePool {
    #[account(mut)]
    pub creator: Signer,

    pub base_mint: InterfaceAccount<Mint>,
    pub quote_mint: InterfaceAccount<Mint>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,

    #[account(
        init,
        payer = creator,
        seeds = [POOL_SEED, &args.index.to_le_bytes(), creator.address().as_ref(), base_mint.address().as_ref(), quote_mint.address().as_ref()],
        bump = args.pool_bump,
    )]
    pub pool: Account<Pool>,

    /// SAFETY: `init` + `mint::decimals`/`mint::authority` below fully
    /// validate and construct this account via a real CPI to Token-2022 —
    /// there is no naclac `Discriminator` to check since this is a raw SPL
    /// `Mint` layout, so `AccountInfo` is correct here, not a gap in
    /// coverage (same reasoning as `pump::create`'s own `mint` field).
    #[account(
        init,
        payer = creator,
        seeds = [POOL_LP_MINT_SEED, pool.address().as_ref()],
        bump = args.lp_mint_bump,
        mint::decimals = LP_MINT_DECIMALS,
        mint::authority = pool,
        token::program = token_2022_program,
    )]
    pub lp_mint: AccountInfo,

    #[account(
        mut,
        associated_token::mint = base_mint,
        associated_token::authority = creator,
        associated_token::bump = args.user_base_token_account_bump,
        token::program = base_token_program,
    )]
    pub user_base_token_account: InterfaceAccount<TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = creator,
        associated_token::bump = args.user_quote_token_account_bump,
        token::program = quote_token_program,
    )]
    pub user_quote_token_account: InterfaceAccount<TokenAccount>,

    /// SAFETY: `init` + `associated_token::mint`/`::authority` below, plus
    /// the real Associated Token Program's own CPI-level address
    /// verification, fully validate and construct this account — there is
    /// no naclac `Discriminator` to check since this is a raw SPL
    /// `TokenAccount` layout, so `AccountInfo` is correct here, not a gap in
    /// coverage (same reasoning as `pump::create`'s own `mint` field).
    #[account(
        init,
        payer = creator,
        associated_token::mint = lp_mint,
        associated_token::authority = creator,
        token::program = token_2022_program,
    )]
    pub user_pool_token_account: AccountInfo,

    /// SAFETY: same as `user_pool_token_account` above. `init_if_needed`
    /// (not strict `init`) — `pump::migrate` pre-creates this itself before
    /// CPI-ing into `create_pool` with `pool_authority` as `creator`
    /// (confirmed via `reference/fee-tier-probe/src/bin/probe31.rs`'s
    /// decoded inner instructions: both this and `pool_quote_token_account`
    /// already exist by the time the real program's nested `CreatePool`
    /// call starts), so a strict `init` would wrongly reject that path.
    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = base_mint,
        associated_token::authority = pool,
        token::program = base_token_program,
    )]
    pub pool_base_token_account: AccountInfo,

    /// SAFETY: same as `pool_base_token_account` above.
    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = quote_mint,
        associated_token::authority = pool,
        token::program = quote_token_program,
    )]
    pub pool_quote_token_account: AccountInfo,

    pub system_program: Program<System>,
    pub token_2022_program: Program<Token2022>,
    pub base_token_program: Interface<TokenInterface>,
    pub quote_token_program: Interface<TokenInterface>,
    pub associated_token_program: Program<AssociatedToken>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result {
    require!(
        ctx.accounts.global_config.disable_flags & DISABLE_CREATE_POOL_FLAG == 0,
        PumpAmmError::PoolCreationDisabled
    );

    let (initial_liquidity, lp_minted) =
        lp_bootstrap_amount(args.base_amount_in, args.quote_amount_in)?;
    let base_mint_decimals = ctx.accounts.base_mint.decimals();
    let quote_mint_decimals = ctx.accounts.quote_mint.decimals();

    // No fee on the initial deposit — full `base_amount_in`/`quote_amount_in`
    // land untouched in the pool's own token accounts (confirmed via
    // `reference/fee-tier-probe/src/bin/probe14.rs` against real bytecode).
    ctx.accounts.base_token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_base_token_account,
            mint: &ctx.accounts.base_mint,
            to: &mut ctx.accounts.pool_base_token_account,
            authority: &ctx.accounts.creator,
        },
        args.base_amount_in,
        base_mint_decimals,
    )?;
    ctx.accounts.quote_token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_quote_token_account,
            mint: &ctx.accounts.quote_mint,
            to: &mut ctx.accounts.pool_quote_token_account,
            authority: &ctx.accounts.creator,
        },
        args.quote_amount_in,
        quote_mint_decimals,
    )?;

    let index_bytes = args.index.to_le_bytes();
    let creator_address = ctx.accounts.creator.address();
    let base_mint_address = ctx.accounts.base_mint.address();
    let quote_mint_address = ctx.accounts.quote_mint.address();
    let pool_signer_seeds: &[&[u8]] = &[
        POOL_SEED,
        &index_bytes,
        creator_address.as_ref(),
        base_mint_address.as_ref(),
        quote_mint_address.as_ref(),
        &[args.pool_bump],
    ];
    let pool_signer: &[&[&[u8]]] = &[pool_signer_seeds];
    ctx.accounts.token_2022_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.lp_mint,
            to: &mut ctx.accounts.user_pool_token_account,
            authority: &ctx.accounts.pool,
        },
        lp_minted,
        pool_signer,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.pool_bump = args.pool_bump;
    pool.index = args.index;
    pool.creator = ctx.accounts.creator.address();
    pool.base_mint = ctx.accounts.base_mint.address();
    pool.quote_mint = ctx.accounts.quote_mint.address();
    pool.lp_mint = ctx.accounts.lp_mint.address();
    pool.pool_base_token_account = ctx.accounts.pool_base_token_account.address();
    pool.pool_quote_token_account = ctx.accounts.pool_quote_token_account.address();
    pool.coin_creator = args.coin_creator;
    pool.lp_supply = lp_minted;
    pool.is_mayhem_mode = args.is_mayhem_mode;
    pool.is_cashback_coin = args.is_cashback_coin;
    pool.virtual_quote_reserves = [0u8; 16];

    let timestamp = unix_timestamp()?;
    emit!(CreatePoolEvent {
        timestamp,
        index: args.index,
        creator: creator_address,
        base_mint: base_mint_address,
        quote_mint: quote_mint_address,
        base_mint_decimals,
        quote_mint_decimals,
        base_amount_in: args.base_amount_in,
        quote_amount_in: args.quote_amount_in,
        pool_base_amount: args.base_amount_in,
        pool_quote_amount: args.quote_amount_in,
        minimum_liquidity: LP_BOOTSTRAP_WITHHELD as u64,
        initial_liquidity,
        lp_token_amount_out: lp_minted,
        pool_bump: args.pool_bump,
        pool: pool.address(),
        lp_mint: ctx.accounts.lp_mint.address(),
        user_base_token_account: ctx.accounts.user_base_token_account.address(),
        user_quote_token_account: ctx.accounts.user_quote_token_account.address(),
        coin_creator: args.coin_creator,
        is_mayhem_mode: args.is_mayhem_mode,
    });

    Ok(())
}
