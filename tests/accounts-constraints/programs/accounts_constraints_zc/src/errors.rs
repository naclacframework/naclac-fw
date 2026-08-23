use naclac_lang::prelude::*;

// Exercises the `field @ CustomError` syntax on relation constraints
// (`naclac-macros/src/instruction/parser.rs`'s `custom_error: Option<syn::Expr>`
// parsed off an `@` suffix, consumed by `security.rs`'s
// `generate_relational_checks`, which emits `#custom_error.into()` instead
// of the default `NaclacError::Unauthorized` when present). Never exercised
// elsewhere in this repo — every other relation test (`related_vault`) only
// takes the default-error path. A single variant is enough to prove the
// codegen actually fires. `#[error_code]` offsets discriminants by 6000
// (`naclac-macros/src/error_code.rs`); with no explicit discriminant this
// variant lands at exactly 6000.
#[error_code]
pub enum VaultError {
    /// the provided authority does not match `vault.admin`
    WrongAdmin,
}
