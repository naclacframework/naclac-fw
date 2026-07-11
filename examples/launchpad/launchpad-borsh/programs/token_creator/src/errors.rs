use naclac_lang::prelude::*;

#[error_code]
pub enum LaunchpadError {
    /// Invalid program ID.
    InvalidProgramId,
    /// Unauthorized action.
    Unauthorized,
    /// Overflow or math error.
    Overflow,
    /// Zero amount provided.
    ZeroAmount,
    /// Mint failed.
    MintFailed,
}
