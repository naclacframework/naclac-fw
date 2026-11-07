//! AMM custom error definitions.

use naclac_lang::prelude::*;

#[error_code]
pub enum AmmError {
    /// Invalid mint for the pool.
    InvalidMint,
    /// Overflow or math error.
    Overflow,
    /// Zero amount provided.
    ZeroAmount,
    /// Slippage limit exceeded.
    SlippageExceeded,
    /// The pool's reserves or LP supply cannot satisfy the request.
    InsufficientLiquidity,
    /// A Token-2022 mint carries an extension the pool does not accept.
    UnsupportedMintExtension,
    /// A swap would lower the product of the pool's reserves.
    InvariantViolated,
}
