//! Entitled payouts, emergency refunds and treasury-directed vault cleanup.

use {
    crate::{
        constants::{REFUND_TIMEOUT_SECONDS, SEED_POOL_VAULT},
        errors::CustomError,
        events::{BetRefunded, RewardClaimed},
        state::{Bet, BetStatus, Pool, PoolStatus, Protocol},
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Withdraw {
    pub user: Signer,
    #[account(mut, constraint = sponsor.address() == pool.created_by @ CustomError::Unauthorized)]
    pub sponsor: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump)]
    pub protocol: Account<Protocol>,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
    #[account(mut, close = sponsor,
        seeds = Bet::seeds(pool.address(), user.address()), bump = bet.bump,
        constraint = bet.user_pubkey == user.address() @ CustomError::Unauthorized,
        constraint = bet.pool_pubkey == pool.address() @ CustomError::PoolMismatch)]
    pub bet: Account<Bet>,
    #[account(constraint = token_mint.address() == pool.stake_token_mint @ CustomError::InvalidMint)]
    pub token_mint: Account<Mint>,
    #[account(mut, seeds = [SEED_POOL_VAULT, pool.address().as_ref()], bump,
        token::mint = token_mint, token::authority = pool)]
    pub pool_vault: Account<TokenAccount>,
    #[account(mut, token::mint = token_mint, token::authority = user,
        constraint = user_token_account.address() != pool_vault.address() @ CustomError::InvalidBet)]
    pub user_token_account: Account<TokenAccount>,
    #[account(mut, token::mint = token_mint, token::authority = protocol.treasury_wallet,
        constraint = treasury_token_account.address() != pool_vault.address() @ CustomError::InvalidTreasury)]
    pub treasury_token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

/// Pays a scored bet from the fixed settlement pot, independent of claim order.
pub fn claim_reward(ctx: Context<Withdraw>) -> Result {
    let pool = &ctx.accounts.pool;
    let bet = &ctx.accounts.bet;
    require!(
        pool.status
            .get()
            .map_err(|_| CustomError::InvalidPoolStatus)?
            == PoolStatus::Resolved,
        CustomError::SettlementTooEarly
    );
    require!(
        bet.status.get().map_err(|_| CustomError::InvalidBet)? == BetStatus::Resolved
            && bet.is_weight_added.get(),
        CustomError::NotCalculatedYet
    );
    let amount = payout_amount(
        pool.total_weight,
        pool.distributable_amount,
        bet.calculated_weight,
        bet.stake,
    )?;
    require!(
        amount <= pool.distributable_amount,
        CustomError::InsufficientLiquidity
    );
    transfer_payout(ctx.accounts, amount)?;
    ctx.accounts.bet.status.set(BetStatus::Claimed);
    ctx.accounts.pool.remaining_participants = ctx
        .accounts
        .pool
        .remaining_participants
        .checked_sub(1)
        .ok_or(CustomError::MathOverflow)?;
    emit!(RewardClaimed {
        bet_address: ctx.accounts.bet.address(),
        user: ctx.accounts.user.address(),
        amount
    });
    finish_if_empty(ctx.accounts)
}

/// Refunds deposited stake after cancellation or an unresolved-pool timeout.
pub fn emergency_refund(ctx: Context<Withdraw>) -> Result {
    let status = ctx
        .accounts
        .pool
        .status
        .get()
        .map_err(|_| CustomError::InvalidPoolStatus)?;
    require!(
        matches!(
            status,
            PoolStatus::Upcoming | PoolStatus::Active | PoolStatus::Closed | PoolStatus::Cancelled
        ),
        CustomError::AlreadyResolved
    );
    require!(
        ctx.accounts
            .bet
            .status
            .get()
            .map_err(|_| CustomError::InvalidBet)?
            == BetStatus::Active,
        CustomError::AlreadyClaimed
    );
    if status != PoolStatus::Cancelled {
        let timeout = ctx
            .accounts
            .bet
            .end_timestamp
            .checked_add(REFUND_TIMEOUT_SECONDS)
            .ok_or(CustomError::MathOverflow)?;
        require!(
            Clock::get()?.unix_timestamp > timeout,
            CustomError::TimeoutNotMet
        );
    }
    let amount = ctx.accounts.bet.stake;
    transfer_payout(ctx.accounts, amount)?;
    ctx.accounts.pool.total_staked = ctx
        .accounts
        .pool
        .total_staked
        .checked_sub(amount)
        .ok_or(CustomError::MathOverflow)?;
    ctx.accounts.pool.total_participants = ctx
        .accounts
        .pool
        .total_participants
        .checked_sub(1)
        .ok_or(CustomError::MathOverflow)?;
    ctx.accounts.pool.remaining_participants = ctx
        .accounts
        .pool
        .remaining_participants
        .checked_sub(1)
        .ok_or(CustomError::MathOverflow)?;
    ctx.accounts.bet.status.set(BetStatus::Claimed);
    emit!(BetRefunded {
        bet_address: ctx.accounts.bet.address(),
        user: ctx.accounts.user.address(),
        amount,
        is_emergency: Bool::from(true)
    });
    finish_if_empty(ctx.accounts)
}

/// Computes only this bet's entitlement; unallocated dust is never a user payout.
fn payout_amount(total_weight: u128, pot: u64, weight: u128, stake: u64) -> Result<u64> {
    if total_weight == 0 {
        return Ok(stake);
    }
    let amount = weight
        .checked_mul(u128::from(pot))
        .ok_or(CustomError::MathOverflow)?
        .checked_div(total_weight)
        .ok_or(CustomError::MathOverflow)?;
    u64::try_from(amount).map_err(|_| CustomError::MathOverflow.into())
}

/// Transfers a user's entitlement with the pool PDA as vault authority.
fn transfer_payout(accounts: &mut Withdraw, amount: u64) -> Result {
    require!(
        amount <= accounts.pool_vault.amount(),
        CustomError::InsufficientLiquidity
    );
    if amount == 0 {
        return Ok(());
    }
    let created_by = accounts.pool.created_by;
    let pool_id = accounts.pool.pool_id;
    let seeds = Pool::seeds(&created_by, &pool_id);
    let bump = [accounts.pool.bump];
    let signer = seeds.signer_seeds(&bump);
    accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut accounts.pool_vault,
            mint: &accounts.token_mint,
            to: &mut accounts.user_token_account,
            authority: &accounts.pool,
        },
        amount,
        accounts.token_mint.decimals(),
        &[signer.signer()],
    )
}

/// After the last entitlement, sends residual tokens to treasury and reclaims rent.
fn finish_if_empty(accounts: &mut Withdraw) -> Result {
    if accounts.pool.remaining_participants != 0 {
        return Ok(());
    }
    empty_vault(
        &mut accounts.pool_vault,
        &accounts.token_mint,
        &mut accounts.treasury_token_account,
        &accounts.pool,
        &mut accounts.sponsor,
        &accounts.token_program,
    )?;
    close_pool_account(&accounts.pool, &mut accounts.sponsor)
}

#[derive(Accounts)]
pub struct ClosePool {
    #[account(mut, constraint = sponsor.address() == pool.created_by @ CustomError::Unauthorized)]
    pub sponsor: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump)]
    pub protocol: Account<Protocol>,
    #[account(mut, close = sponsor,
        seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
    #[account(constraint = token_mint.address() == pool.stake_token_mint @ CustomError::InvalidMint)]
    pub token_mint: Account<Mint>,
    #[account(mut, seeds = [SEED_POOL_VAULT, pool.address().as_ref()], bump,
        token::mint = token_mint, token::authority = pool)]
    pub pool_vault: Account<TokenAccount>,
    #[account(mut, token::mint = token_mint, token::authority = protocol.treasury_wallet,
        constraint = treasury_token_account.address() != pool_vault.address() @ CustomError::InvalidTreasury)]
    pub treasury_token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

/// Reclaims an empty cancelled or settled pool that has no claimant to close it.
pub fn close_pool(ctx: Context<ClosePool>) -> Result {
    require!(
        ctx.accounts.pool.remaining_participants == 0,
        CustomError::WeightsIncomplete
    );
    let status = ctx
        .accounts
        .pool
        .status
        .get()
        .map_err(|_| CustomError::InvalidPoolStatus)?;
    require!(
        matches!(status, PoolStatus::Cancelled | PoolStatus::Resolved),
        CustomError::SettlementTooEarly
    );
    empty_vault(
        &mut ctx.accounts.pool_vault,
        &ctx.accounts.token_mint,
        &mut ctx.accounts.treasury_token_account,
        &ctx.accounts.pool,
        &mut ctx.accounts.sponsor,
        &ctx.accounts.token_program,
    )
}

/// Empties a vault into treasury before closing its token account.
fn empty_vault(
    vault: &mut Account<TokenAccount>,
    mint: &Account<Mint>,
    treasury: &mut Account<TokenAccount>,
    pool: &Account<Pool>,
    sponsor: &mut Signer,
    token_program: &Program<Token>,
) -> Result {
    let created_by = pool.created_by;
    let pool_id = pool.pool_id;
    let seeds = Pool::seeds(&created_by, &pool_id);
    let bump = [pool.bump];
    let signer = seeds.signer_seeds(&bump);
    let remaining = vault.amount();
    if remaining > 0 {
        token_program.transfer_checked_signed(
            TransferCheckedAccounts {
                from: vault,
                mint,
                to: treasury,
                authority: pool,
            },
            remaining,
            mint.decimals(),
            &[signer.signer()],
        )?;
    }
    token_program.close_account_signed(
        CloseAccountAccounts {
            account: vault,
            destination: sponsor,
            authority: pool,
        },
        &[signer.signer()],
    )
}

/// Closes only the program-owned pool, after all token CPIs have finished using it.
fn close_pool_account(pool: &Account<Pool>, sponsor: &mut Signer) -> Result {
    let info = pool.to_account_info();
    require!(
        info.owner() == crate::ID && info.address() != sponsor.address(),
        CustomError::Unauthorized
    );
    let lamports = info.lamports();
    sponsor.to_account_info().add_lamports(lamports)?;
    info.sub_lamports(lamports)?;
    info.resize(0)?;
    let mut view = info.view;
    unsafe {
        view.assign(SYSTEM_PROGRAM_ID.as_address());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_order_does_not_change_entitlements() {
        assert_eq!(payout_amount(3, 101, 2, 50).unwrap(), 67);
        assert_eq!(payout_amount(3, 101, 1, 50).unwrap(), 33);
        assert_eq!(payout_amount(3, 101, 0, 50).unwrap(), 0);
    }

    #[test]
    fn no_winners_refunds_only_deposited_stake() {
        assert_eq!(payout_amount(0, 999, 0, 50).unwrap(), 50);
    }

    #[test]
    fn payout_overflow_is_an_error() {
        assert!(payout_amount(u128::MAX, u64::MAX, u128::MAX, 1).is_err());
        assert!(payout_amount(1, u64::MAX, 2, 1).is_err());
    }
}
