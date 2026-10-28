//! `#[error_code]` enums: implicit codes, explicit discriminants, doc-comment messages.

use naclac_lang::prelude::*;

#[error_code]
pub enum VaultError {
    /// The caller is not the admin.
    NotAdmin,
    Plain,
    /// Explicit code.
    Explicit = 40,
    /// First line.
    /// Second line.
    MultiLine,
    AfterGap,
}

/// Its own code range, clear of `VaultError`'s.
#[error_code(offset = 6100)]
pub enum SecondError {
    A,
    B = 100,
    C,
}
