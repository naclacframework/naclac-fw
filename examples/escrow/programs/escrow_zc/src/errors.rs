use naclac_lang::prelude::*;
#[error_code]
pub enum EscrowError {
    /// Invalid mint for the trade.
    InvalidMint,
    /// Unauthorized action.
    Unauthorized,
    /// Overflow or math error.
    Overflow,
    /// The amount provided must be greater than zero.
    ZeroAmount,
}