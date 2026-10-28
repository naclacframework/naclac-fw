//! Child of a `#[path]`-loaded module.

use naclac_lang::prelude::*;

#[error_code(offset = 6100)]
pub enum MovedError {
    /// Moved error.
    Gone,
}
