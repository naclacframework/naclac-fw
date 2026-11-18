//! Public pool creation, resolution and weight settlement.

use {
    crate::{
        constants::{MAX_TITLE_LENGTH, SEED_POOL_VAULT},
        errors::CustomError,
        events::PoolCreated,
        state::{Bet, BetStatus, Pool, PoolStatus, Protocol},
        utils::math::{
            calculate_accuracy_score, calculate_conviction_bonus, calculate_time_bonus,
            calculate_weight,
        },
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CreatePool {
    #[account(mut)]
    pub created_by: Signer,
    #[account(mut, seeds = Protocol::seeds(), bump = protocol.bump,
        constraint = protocol.admin == created_by.address() @ CustomError::Unauthorized)]
    pub protocol: Account<Protocol>,
    #[account(init, payer = created_by,
        seeds = Pool::seeds(created_by.address(), protocol.total_pools), bump)]
    pub pool: Account<Pool>,
    pub token_mint: Account<Mint>,
    #[account(init, payer = created_by,
        seeds = [SEED_POOL_VAULT, pool.address().as_ref()], bump,
        token::mint = token_mint, token::authority = pool)]
    pub pool_vault: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

/// Creates a pool and a PDA-owned vault for its staking mint.
pub fn create_pool(
    ctx: Context<CreatePool>,
    title: FixedString<MAX_TITLE_LENGTH>,
    start_time: i64,
    end_time: i64,
    max_accuracy_buffer: u64,
    conviction_bonus_bps: u64,
) -> Result {
    let now = Clock::get()?.unix_timestamp;
    title.as_str().map_err(|_| CustomError::InvalidTitle)?;
    let duration = end_time
        .checked_sub(start_time)
        .ok_or(CustomError::MathOverflow)?;
    require!(
        duration >= 10 && end_time > now,
        CustomError::DurationTooShort
    );
    let cutoff_duration = (duration / 20).clamp(10, 120);
    let cutoff_time = end_time
        .checked_sub(cutoff_duration)
        .ok_or(CustomError::MathOverflow)?;
    require!(cutoff_time > start_time, CustomError::InvalidTime);
    let pool = &mut ctx.accounts.pool;
    pool.created_by = ctx.accounts.created_by.address();
    pool.title = title;
    pool.pool_id = ctx.accounts.protocol.total_pools;
    pool.stake_token_mint = ctx.accounts.token_mint.address();
    pool.start_time = start_time;
    pool.end_time = end_time;
    pool.cutoff_time = cutoff_time;
    pool.max_accuracy_buffer = max_accuracy_buffer;
    pool.conviction_bonus_bps = conviction_bonus_bps;
    pool.bump = ctx.bumps.pool;
    pool.status.set(if now < start_time {
        PoolStatus::Upcoming
    } else {
        PoolStatus::Active
    });
    ctx.accounts.protocol.total_pools = ctx
        .accounts
        .protocol
        .total_pools
        .checked_add(1)
        .ok_or(CustomError::MathOverflow)?;
    let mut pool_name = [0; MAX_TITLE_LENGTH];
    pool_name[..title.len()].copy_from_slice(title.as_bytes());
    emit!(PoolCreated {
        pool_name,
        pool_name_len: title.len() as u16,
        start_time,
        end_time
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ManagePool {
    pub admin: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump,
        constraint = protocol.admin == admin.address() @ CustomError::Unauthorized)]
    pub protocol: Account<Protocol>,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
}

/// Begins a single resolution after the pool's end time.
pub fn resolve_pool(ctx: Context<ManagePool>, final_outcome: u64) -> Result {
    let pool = &mut ctx.accounts.pool;
    let status = pool
        .status
        .get()
        .map_err(|_| CustomError::InvalidPoolStatus)?;
    require!(
        matches!(
            status,
            PoolStatus::Upcoming | PoolStatus::Active | PoolStatus::Closed
        ),
        CustomError::AlreadyResolved
    );
    let now = Clock::get()?.unix_timestamp;
    require!(now >= pool.end_time, CustomError::DurationTooShort);
    pool.resolution_result = final_outcome;
    pool.resolution_ts = now;
    pool.remaining_participants = pool.total_participants;
    pool.status.set(PoolStatus::Resolving);
    Ok(())
}

/// Cancels a pool before resolution so participants can reclaim their stakes.
pub fn cancel_pool(ctx: Context<ManagePool>) -> Result {
    let pool = &mut ctx.accounts.pool;
    let status = pool
        .status
        .get()
        .map_err(|_| CustomError::InvalidPoolStatus)?;
    require!(
        matches!(
            status,
            PoolStatus::Upcoming | PoolStatus::Active | PoolStatus::Closed
        ),
        CustomError::PoolNotCancellable
    );
    pool.status.set(PoolStatus::Cancelled);
    Ok(())
}

#[derive(Accounts)]
pub struct BatchCalculateWeights {
    pub admin: Signer,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
    pub pool: Account<Pool>,
}

/// Scores writable, program-owned bet PDAs exactly once against a fixed resolution.
pub fn batch_calculate_weights(ctx: Context<BatchCalculateWeights>) -> Result {
    let pool = &mut ctx.accounts.pool;
    require!(
        pool.status
            .get()
            .map_err(|_| CustomError::InvalidPoolStatus)?
            == PoolStatus::Resolving,
        CustomError::SettlementTooEarly
    );
    let pool_key = pool.address();
    for (index, info) in ctx.remaining_accounts.iter().enumerate() {
        require!(
            info.owner() == *ctx.program_id && info.is_writable(),
            CustomError::InvalidBet
        );
        let mut bet = Account::<Bet>::try_from_mut(info, index + 2)?;
        require!(bet.pool_pubkey == pool_key, CustomError::PoolMismatch);
        let user = bet.user_pubkey;
        let seeds = Bet::seeds(&pool_key, &user);
        let bump = [bet.bump];
        let expected =
            Address::create_program_address(&seeds.as_slices_with_bump(&bump), ctx.program_id)
                .map_err(|_| CustomError::SeedMismatch)?;
        require!(expected == info.address(), CustomError::SeedMismatch);
        let status = bet.status.get().map_err(|_| CustomError::InvalidBet)?;
        if status == BetStatus::Resolved {
            require!(bet.is_weight_added.get(), CustomError::InvalidBet);
            continue;
        }
        require!(
            status == BetStatus::Active && !bet.is_weight_added.get(),
            CustomError::InvalidBet
        );
        let accuracy = calculate_accuracy_score(
            bet.prediction,
            pool.resolution_result,
            pool.max_accuracy_buffer,
        )?;
        let time = calculate_time_bonus(pool.start_time, pool.cutoff_time, bet.creation_ts)?;
        let weight = calculate_weight(
            bet.stake,
            accuracy,
            time,
            calculate_conviction_bonus(bet.update_count),
        )?;
        pool.total_weight = pool
            .total_weight
            .checked_add(weight)
            .ok_or(CustomError::MathOverflow)?;
        pool.weights_calculated_count = pool
            .weights_calculated_count
            .checked_add(1)
            .ok_or(CustomError::MathOverflow)?;
        require!(
            pool.weights_calculated_count <= pool.total_participants,
            CustomError::WeightsIncomplete
        );
        bet.calculated_weight = weight;
        bet.is_weight_added.set(true);
        bet.status.set(BetStatus::Resolved);
    }
    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeWeights {
    pub admin: Signer,
    #[account(seeds = Protocol::seeds(), bump = protocol.bump)]
    pub protocol: Account<Protocol>,
    #[account(mut, seeds = Pool::seeds(pool.created_by, pool.pool_id), bump = pool.bump)]
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

/// Finalizes fully scored pools and transfers the configured fee to the treasury.
pub fn finalize_weights(ctx: Context<FinalizeWeights>) -> Result {
    let pool = &mut ctx.accounts.pool;
    let protocol = &ctx.accounts.protocol;
    require!(
        pool.status
            .get()
            .map_err(|_| CustomError::InvalidPoolStatus)?
            == PoolStatus::Resolving,
        CustomError::SettlementTooEarly
    );
    require!(
        pool.weights_calculated_count == pool.total_participants,
        CustomError::WeightsIncomplete
    );
    let ready = pool
        .resolution_ts
        .checked_add(protocol.batch_settle_wait_duration)
        .ok_or(CustomError::MathOverflow)?;
    require!(
        Clock::get()?.unix_timestamp >= ready,
        CustomError::SettlementTooEarly
    );
    if pool.total_participants <= 1 {
        pool.total_weight = 0;
    }
    let total_assets = ctx.accounts.pool_vault.amount();
    let fee =
        if protocol.protocol_fee_bps > 0 && pool.total_participants > 1 && pool.total_weight > 0 {
            let amount = u128::from(total_assets)
                .checked_mul(u128::from(protocol.protocol_fee_bps))
                .ok_or(CustomError::MathOverflow)?
                / 10_000;
            u64::try_from(amount).map_err(|_| CustomError::MathOverflow)?
        } else {
            0
        };
    let created_by = pool.created_by;
    let pool_id = pool.pool_id;
    let seeds = Pool::seeds(&created_by, &pool_id);
    let bump = [pool.bump];
    let signer = seeds.signer_seeds(&bump);
    if fee > 0 {
        ctx.accounts.token_program.transfer_checked_signed(
            TransferCheckedAccounts {
                from: &mut ctx.accounts.pool_vault,
                mint: &ctx.accounts.token_mint,
                to: &mut ctx.accounts.treasury_token_account,
                authority: pool,
            },
            fee,
            ctx.accounts.token_mint.decimals(),
            &[signer.signer()],
        )?;
    }
    pool.distributable_amount = total_assets
        .checked_sub(fee)
        .ok_or(CustomError::MathOverflow)?;
    pool.status.set(PoolStatus::Resolved);
    Ok(())
}
