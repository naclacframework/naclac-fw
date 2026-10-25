#![no_std]

#[cfg(all(feature = "solana", feature = "pinocchio"))]
compile_error!(
    "naclac-lang: the `solana` and `pinocchio` runtimes are mutually exclusive. `pinocchio` is \
     the default; a program that selects `solana` must set `default-features = false` on its \
     `naclac-lang` dependency."
);

pub mod prelude;

#[cfg(not(feature = "pinocchio"))]
pub use naclac_core::naclac_solana_entrypoint;
pub use naclac_core::*;
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub use naclac_core::{__naclac_panic_handler, __naclac_panic_hook};
pub use naclac_core::{context, cpi, cursor, error, event, system_program, wrappers};

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
pub use naclac_token::{associated_token, constraints as token_constraints, token};

#[cfg(all(
    feature = "metadata",
    not(any(feature = "solana", feature = "pinocchio"))
))]
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
