#![no_std]

pub mod prelude;

#[cfg(not(feature = "pinocchio"))]
pub use naclac_core::naclac_solana_entrypoint;
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub use naclac_core::{__naclac_panic_handler, __naclac_panic_hook};
pub use naclac_core::{context, cpi, cursor, error, event, system_program, wrappers, *};

/// Panic support for a program whose own crate is `no_std`. On the Solana
/// backend `token` links the SPL crates, which use `std`, so std's panic
/// handler is present and the program supplies only the hook.
#[cfg(all(feature = "token", not(feature = "pinocchio")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_nostd_panic {
    () => {
        $crate::__naclac_panic_hook!();
    };
}

/// Panic support for a program whose own crate is `no_std` and whose build
/// has no `std` from naclac: the program's `#[panic_handler]`.
#[cfg(not(all(feature = "token", not(feature = "pinocchio"))))]
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_nostd_panic {
    () => {
        $crate::__naclac_panic_handler!();
    };
}

#[cfg(all(feature = "token", not(any(feature = "solana", feature = "pinocchio"))))]
compile_error!("the `token` feature requires `solana` or `pinocchio` to also be enabled");

#[cfg(feature = "token")]
pub use naclac_token::{associated_token, token};

#[cfg(all(
    feature = "metadata",
    not(any(feature = "solana", feature = "pinocchio"))
))]
compile_error!("the `metadata` feature requires `solana` or `pinocchio` to also be enabled");

/// Re-exported so `naclac-macros`' generated `idl-build` code
/// (`naclac_lang::naclac_idl::...`) resolves in the calling program crate —
/// a proc-macro's own dependencies (naclac-macros') are never visible to the
/// code it generates, only this crate's are.
#[cfg(feature = "idl-build")]
pub use naclac_idl;
#[cfg(feature = "metadata")]
pub use naclac_metadata as metadata;
/// Same reasoning as `naclac_idl` just above, for `naclac_lang::naclac_syn::...`
/// paths the generated `idl-build` assembler also references directly.
#[cfg(feature = "idl-build")]
pub use naclac_syn;
