use naclac_lang::prelude::*;

#[error_code]
pub enum PumpAmmError {
    /// Non-native quote mints aren't supported here
    UnsupportedQuoteMint,
    /// Pool creation is disabled
    PoolCreationDisabled,
    /// Math overflow
    MathOverflow,
    /// Base amount cannot be zero
    ZeroBaseAmount,
    /// Quote amount cannot be zero
    ZeroQuoteAmount,
    /// Not enough liquidity for pool bootstrap
    TooLittlePoolTokenLiquidity,
    /// Invalid admin
    InvalidAdmin,
    /// Boost is disabled
    BoostDisabled,
    /// Pool cannot be boosted
    PoolCannotBoost,
}
