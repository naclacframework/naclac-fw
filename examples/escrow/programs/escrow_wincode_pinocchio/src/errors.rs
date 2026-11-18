use naclac_lang::prelude::*;
#[error_code]
pub enum EscrowError {
    /// Invalid mint for the trade.
    InvalidMint,
    /// The amount provided must be greater than zero.
    ZeroAmount,
}
