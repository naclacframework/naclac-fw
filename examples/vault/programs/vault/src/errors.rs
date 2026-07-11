use naclac_lang::prelude::*;

#[error_code]
pub enum VaultError {
    /// User is not authorized to perform this action.
    Unauthorized,
    /// User does not have enough funds to perform this action.
    InsufficientFunds,
}
