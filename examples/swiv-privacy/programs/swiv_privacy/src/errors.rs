//! Public errors for protocol administration and pool settlement.

use naclac_lang::prelude::*;

#[error_code]
pub enum CustomError {
    #[msg("Global protocol is paused.")]
    Paused,
    #[msg("Unauthorized admin action.")]
    Unauthorized,
    #[msg("Math operation overflow.")]
    MathOverflow,
    #[msg("Insufficient liquidity in pool.")]
    InsufficientLiquidity,
    #[msg("Bet is already claimed.")]
    AlreadyClaimed,
    #[msg("Pool is already resolved.")]
    AlreadyResolved,
    #[msg("Weights have already been finalized.")]
    WeightsAlreadyFinalized,
    #[msg("Bet is already initialized.")]
    BetAlreadyInitialized,
    #[msg("Bet duration is too short.")]
    DurationTooShort,
    #[msg("Invalid asset symbol.")]
    InvalidAsset,
    #[msg("Asset is not whitelisted.")]
    AssetNotWhitelisted,
    #[msg("Seeds do not result in a valid address.")]
    SeedMismatch,
    #[msg("Bet does not match the current pool configuration.")]
    PoolMismatch,
    #[msg("Admin force-settlement is not yet allowed for this bet.")]
    SettlementTooEarly,
    #[msg("Emergency refund timeout has not been met.")]
    TimeoutNotMet,
    #[msg("Bet weight has not been calculated yet.")]
    NotCalculatedYet,
    #[msg("The pool must end before this operation is allowed.")]
    UndelegationTooEarly,
    #[msg("Market is closed for new predictions.")]
    MarketClosed,
    #[msg("Pool cannot be cancelled once resolution has started.")]
    PoolNotCancellable,
    #[msg("Not all bets have been scored yet.")]
    WeightsIncomplete,
    #[msg("Protocol fee exceeds the maximum allowed value.")]
    InvalidFee,
    #[msg("New admin must not be the zero address.")]
    InvalidAdmin,
    #[msg("Invalid timestamp or duration.")]
    InvalidTime,
    #[msg("Invalid token amount.")]
    InvalidAmount,
    #[msg("Invalid treasury wallet or token account.")]
    InvalidTreasury,
    #[msg("Invalid token mint.")]
    InvalidMint,
    #[msg("Invalid pool title.")]
    InvalidTitle,
    #[msg("Optional value has an invalid tag.")]
    InvalidOptionalValue,
    #[msg("Invalid bet account or status.")]
    InvalidBet,
    #[msg("Invalid pool status.")]
    InvalidPoolStatus,
}
