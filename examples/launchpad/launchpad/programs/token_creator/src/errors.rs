//! Token creator custom error definitions.

use naclac_lang::prelude::*;

#[error_code]
pub enum LaunchpadError {
    /// The pool and launcher token amounts add up past `u64::MAX`.
    Overflow,
}
