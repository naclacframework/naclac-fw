#![cfg_attr(
    all(any(feature = "pinocchio", feature = "no-std"), not(feature = "idl-build")),
    no_std
)]

pub mod prelude;

pub use naclac_core::*;
pub use naclac_core::{context, cpi, cursor, error, event, system_program, wrappers};

#[cfg(all(feature = "token", not(any(feature = "solana", feature = "pinocchio"))))]
compile_error!("the `token` feature requires `solana` or `pinocchio` to also be enabled");

#[cfg(feature = "token")]
pub use naclac_token::{associated_token, token};

#[cfg(all(feature = "metadata", not(any(feature = "solana", feature = "pinocchio"))))]
compile_error!("the `metadata` feature requires `solana` or `pinocchio` to also be enabled");

#[cfg(feature = "metadata")]
pub use naclac_metadata as metadata;

/// Re-exported so `naclac-macros`' generated `idl-build` code
/// (`naclac_lang::naclac_idl::...`) resolves in the calling program crate —
/// a proc-macro's own dependencies (naclac-macros') are never visible to the
/// code it generates, only this crate's are.
#[cfg(feature = "idl-build")]
pub use naclac_idl;

/// Same reasoning as `naclac_idl` just above, for `naclac_lang::naclac_syn::...`
/// paths the generated `idl-build` assembler also references directly.
#[cfg(feature = "idl-build")]
pub use naclac_syn;
