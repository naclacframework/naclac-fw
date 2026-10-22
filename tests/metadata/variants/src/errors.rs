use naclac_lang::prelude::*;

#[error_code]
pub enum CounterError {
    /// User is not authorized to perform this action.
    Unauthorized,
}
