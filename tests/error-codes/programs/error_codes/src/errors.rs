use naclac_lang::prelude::*;

// `#[error_code]` offsets discriminants by 6000 (naclac-macros/src/error_code.rs)
// specifically to stay clear of NaclacError's own framework codespace
// (3000 + index*100 + error_type). With no explicit discriminants, these
// three variants land at exactly 6000, 6001, 6002.
#[error_code]
pub enum TestError {
    /// amount must be greater than zero
    ZeroAmount,
    /// caller does not match the required authority
    Unauthorized,
    /// amount exceeds the maximum allowed
    TooLarge,
}
