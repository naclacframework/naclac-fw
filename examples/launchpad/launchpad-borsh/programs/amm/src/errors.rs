use naclac_lang::prelude::*;

#[error_code]
pub enum AmmError {
    /// Invalid mint for the pool.
    InvalidMint,
    /// Unauthorized action.
    Unauthorized,
    /// Overflow or math error.
    Overflow,
    /// Zero amount provided.
    ZeroAmount,
    /// Slippage limit exceeded.
    SlippageExceeded,
    /// The pool is already initialized.
    AlreadyInitialized,
}
