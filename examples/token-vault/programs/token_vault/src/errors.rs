use naclac_lang::prelude::*;

#[error_code]
pub enum VaultError {
    /// User is not authorized to perform this action.
    Unauthorized,
    /// Insufficient funds to perform this withdrawal.
    InsufficientFunds,
    /// Invalid token mint address.
    InvalidMint,
    /// Invalid token account owner.
    InvalidOwner,
}
