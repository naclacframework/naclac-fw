//! Public predictions and atomic token-backed stake changes.

use {
    crate::{
        constants::SEED_POOL_VAULT,
        errors::CustomError,
        state::{Bet, BetStatus, Pool, PoolStatus, Protocol},
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitBet {
    pub user: Signer,
    #[account(mut, constraint = sponsor.address() == protocol.admin @ CustomError::Unauthorized)]
    pub sponsor: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump,
        constraint = !protocol.paused.get() @ CustomError::Paused)]
    pub protocol: Account<Protocol>,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
    #[account(constraint = token_mint.address() == pool.stake_token_mint @ CustomError::InvalidMint)]
    pub token_mint: Account<Mint>,
    #[account(mut, seeds = [SEED_POOL_VAULT, pool.address().as_ref()], bump,
        token::mint = token_mint, token::authority = pool)]
    pub pool_vault: Account<TokenAccount>,
    #[account(mut, token::mint = token_mint, token::authority = user)]
    pub user_token_account: Account<TokenAccount>,
    #[account(init, payer = sponsor, seeds = Bet::seeds(pool.address(), user.address()), bump)]
    pub bet: Account<Bet>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

/// Deposits a stake and creates the user's single bet PDA for this pool.
pub fn init_bet(ctx: Context<InitBet>, amount: u64) -> Result {
    let now = Clock::get()?.unix_timestamp;
    require_open(&ctx.accounts.pool, now)?;
    require!(amount > 0, CustomError::InvalidAmount);
    ctx.accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.user_token_account,
            mint: &ctx.accounts.token_mint,
            to: &mut ctx.accounts.pool_vault,
            authority: &ctx.accounts.user,
        },
        amount,
        ctx.accounts.token_mint.decimals(),
    )?;
    let pool = &mut ctx.accounts.pool;
    pool.total_staked = pool
        .total_staked
        .checked_add(amount)
        .ok_or(CustomError::MathOverflow)?;
    pool.total_participants = pool
        .total_participants
        .checked_add(1)
        .ok_or(CustomError::MathOverflow)?;
    pool.remaining_participants = pool
        .remaining_participants
        .checked_add(1)
        .ok_or(CustomError::MathOverflow)?;
    let bet = &mut ctx.accounts.bet;
    bet.user_pubkey = ctx.accounts.user.address();
    bet.pool_pubkey = pool.address();
    bet.stake = amount;
    bet.end_timestamp = pool.end_time;
    bet.creation_ts = now;
    bet.update_count = 0;
    bet.calculated_weight = 0;
    bet.is_weight_added.set(false);
    bet.prediction = 0;
    bet.status.set(BetStatus::Active);
    bet.bump = ctx.bumps.bet;
    Ok(())
}

#[derive(Accounts)]
pub struct PlaceBet {
    pub user: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump,
        constraint = !protocol.paused.get() @ CustomError::Paused)]
    pub protocol: Account<Protocol>,
    #[account(seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
    #[account(mut, seeds = Bet::seeds(pool.address(), user.address()), bump = bet.bump,
        constraint = bet.pool_pubkey == pool.address() @ CustomError::PoolMismatch,
        constraint = bet.user_pubkey == user.address() @ CustomError::Unauthorized)]
    pub bet: Account<Bet>,
}

/// Records a public prediction and counts it as a prediction update.
pub fn place_bet(ctx: Context<PlaceBet>, prediction: u64) -> Result {
    require_open(&ctx.accounts.pool, Clock::get()?.unix_timestamp)?;
    let bet = &mut ctx.accounts.bet;
    require!(
        bet.status.get().map_err(|_| CustomError::InvalidBet)? == BetStatus::Active,
        CustomError::AlreadyClaimed
    );
    bet.prediction = prediction;
    bet.update_count = bet
        .update_count
        .checked_add(1)
        .ok_or(CustomError::MathOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateBet {
    pub user: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump,
        constraint = !protocol.paused.get() @ CustomError::Paused)]
    pub protocol: Account<Protocol>,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
    #[account(mut, seeds = Bet::seeds(pool.address(), user.address()), bump = bet.bump,
        constraint = bet.pool_pubkey == pool.address() @ CustomError::PoolMismatch,
        constraint = bet.user_pubkey == user.address() @ CustomError::Unauthorized)]
    pub bet: Account<Bet>,
    #[account(constraint = token_mint.address() == pool.stake_token_mint @ CustomError::InvalidMint)]
    pub token_mint: Account<Mint>,
    #[account(mut, seeds = [SEED_POOL_VAULT, pool.address().as_ref()], bump,
        token::mint = token_mint, token::authority = pool)]
    pub pool_vault: Account<TokenAccount>,
    #[account(mut, token::mint = token_mint, token::authority = user)]
    pub user_token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

/// Updates a prediction and transfers any added stake in the same instruction.
pub fn update_bet(ctx: Context<UpdateBet>, new_prediction: u64, additional_stake: u64) -> Result {
    require_open(&ctx.accounts.pool, Clock::get()?.unix_timestamp)?;
    require!(
        ctx.accounts
            .bet
            .status
            .get()
            .map_err(|_| CustomError::InvalidBet)?
            == BetStatus::Active,
        CustomError::AlreadyClaimed
    );
    deposit(ctx.accounts, additional_stake)?;
    ctx.accounts.bet.prediction = new_prediction;
    ctx.accounts.bet.update_count = ctx
        .accounts
        .bet
        .update_count
        .checked_add(1)
        .ok_or(CustomError::MathOverflow)?;
    Ok(())
}

/// Increases deposited and recorded stake without changing the prediction.
pub fn add_stake(ctx: Context<UpdateBet>, amount: u64) -> Result {
    require_open(&ctx.accounts.pool, Clock::get()?.unix_timestamp)?;
    require!(
        ctx.accounts
            .bet
            .status
            .get()
            .map_err(|_| CustomError::InvalidBet)?
            == BetStatus::Active,
        CustomError::AlreadyClaimed
    );
    require!(amount > 0, CustomError::InvalidAmount);
    deposit(ctx.accounts, amount)
}

/// Rejects entries outside the pool's live betting interval or lifecycle.
fn require_open(pool: &Pool, now: i64) -> Result {
    let status = pool
        .status
        .get()
        .map_err(|_| CustomError::InvalidPoolStatus)?;
    require!(
        matches!(status, PoolStatus::Upcoming | PoolStatus::Active),
        CustomError::MarketClosed
    );
    require!(now >= pool.start_time, CustomError::DurationTooShort);
    require!(now < pool.cutoff_time, CustomError::MarketClosed);
    Ok(())
}

/// Keeps the bet's recorded stake and pool accounting backed by a token transfer.
fn deposit(accounts: &mut UpdateBet, amount: u64) -> Result {
    if amount == 0 {
        return Ok(());
    }
    accounts.token_program.transfer_checked(
        TransferCheckedAccounts {
            from: &mut accounts.user_token_account,
            mint: &accounts.token_mint,
            to: &mut accounts.pool_vault,
            authority: &accounts.user,
        },
        amount,
        accounts.token_mint.decimals(),
    )?;
    accounts.bet.stake = accounts
        .bet
        .stake
        .checked_add(amount)
        .ok_or(CustomError::MathOverflow)?;
    accounts.pool.total_staked = accounts
        .pool
        .total_staked
        .checked_add(amount)
        .ok_or(CustomError::MathOverflow)?;
    Ok(())
}
