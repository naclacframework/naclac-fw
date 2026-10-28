//! # Naclac Language Library
//!
//! The `naclac-lang` crate provides the runtime abstractions, structural types, and zero-copy
//! pointer management utilities required by the `naclac-macros` engine.
//!
//! ## Execution Mode Abstraction
//! A program picks one runtime and one account-data layout, independently:
//! - Runtime: `solana` (the SDK's `AccountInfo`) or `pinocchio` (`AccountView`, `no_std`).
//! - Layout: `bytemuck` (zero-copy, via `Pod` and `Zeroable`), `wincode`, or `borsh`
//!   (the last two keep heap-allocated types such as `Vec` and `String`; `borsh`
//!   is available on the `solana` runtime only).

#![no_std]

extern crate alloc;
// Off the Solana target (clippy, tests, the IDL build), linking `std` gives a
// `no_std` program crate the panic handler and allocator a host build needs.
#[cfg(not(target_os = "solana"))]
extern crate std;

// Solana backend
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_account_info;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_address;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_clock;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_instruction;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_msg;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_program_error;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_rent;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_sha256_hasher;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_system_interface;

#[cfg(not(any(feature = "solana", feature = "pinocchio")))]
compile_error!(
    "naclac-core: select a runtime with the `solana` or the `pinocchio` feature (through \
     `naclac-lang`, `pinocchio` is the default)."
);
// A program selects exactly one account-data layout: `bytemuck` (zero-copy),
// `borsh` or `wincode`. `naclac-macros` emits all three representations,
// each behind its own layout feature in the program's crate, so two active
// layouts would compile two definitions of the same account into one
// program. Cargo features only ever turn on, never off, so a dependency edge
// can enable a second layout without the program asking for it.
#[cfg(all(feature = "borsh", feature = "wincode"))]
compile_error!(
    "naclac-core: the `borsh` and `wincode` features are mutually exclusive — a program \
     selects exactly one account-data layout (`bytemuck`, `borsh` or `wincode`). Both ended \
     up active on this build at once, which usually means a dependency enabled one of them \
     without your program's own Cargo.toml asking for it (check `cargo tree -e features` for \
     which edge pulled it in)."
);
#[cfg(all(feature = "bytemuck", feature = "borsh"))]
compile_error!(
    "naclac-core: the `bytemuck` and `borsh` features are mutually exclusive — a program \
     selects exactly one account-data layout (`bytemuck`, `borsh` or `wincode`). If this \
     program is not zero-copy, turn off default features on `naclac-lang`."
);
#[cfg(all(feature = "bytemuck", feature = "wincode"))]
compile_error!(
    "naclac-core: the `bytemuck` and `wincode` features are mutually exclusive — a program \
     selects exactly one account-data layout (`bytemuck`, `borsh` or `wincode`). If this \
     program is not zero-copy, turn off default features on `naclac-lang`."
);

// Borsh needs a heap-allocating decoder and has no implementation for the
// Pinocchio runtime; use `wincode` for heap-allocated accounts there.
#[cfg(all(feature = "borsh", feature = "pinocchio"))]
compile_error!(
    "naclac-core: the `borsh` feature is not available with the `pinocchio` runtime. Use \
     `wincode` for heap-allocated accounts on Pinocchio, or `bytemuck` for zero-copy; to keep \
     `borsh`, use the `solana` runtime."
);

// Markers that `#[program]` references from the program's own crate for each
// runtime and layout feature the program declares. A feature declared in the
// program's `[features]` but not forwarded to `naclac-lang` then fails to
// compile at the missing name below, instead of as a long list of unrelated
// errors.
#[cfg(feature = "solana")]
#[doc(hidden)]
pub const __NACLAC_LANG_HAS_SOLANA_RUNTIME: bool = true;
#[cfg(feature = "pinocchio")]
#[doc(hidden)]
pub const __NACLAC_LANG_HAS_PINOCCHIO_RUNTIME: bool = true;
#[cfg(feature = "bytemuck")]
#[doc(hidden)]
pub const __NACLAC_LANG_HAS_BYTEMUCK_LAYOUT: bool = true;
#[cfg(feature = "borsh")]
#[doc(hidden)]
pub const __NACLAC_LANG_HAS_BORSH_LAYOUT: bool = true;
#[cfg(feature = "wincode")]
#[doc(hidden)]
pub const __NACLAC_LANG_HAS_WINCODE_LAYOUT: bool = true;

#[cfg(feature = "borsh")]
pub extern crate borsh;
#[cfg(feature = "wincode")]
pub extern crate wincode;

#[cfg(feature = "bytemuck")]
pub extern crate bytemuck;

// Pinocchio backend
#[cfg(feature = "pinocchio")]
pub use pinocchio;
#[cfg(feature = "pinocchio")]
pub use pinocchio_log;
#[cfg(feature = "pinocchio")]
pub use pinocchio_system;

// ===========================================================================
// Modules
// ===========================================================================

pub mod address_literal;
pub mod base58;
pub mod constraint;
pub mod context;
pub mod cpi;
#[cfg(not(feature = "pinocchio"))]
pub mod entrypoint;
pub mod error;
pub mod event;
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod panic;
pub mod prelude;
#[cfg(feature = "wincode")]
pub mod wincode_config;
pub mod wrappers;

// Account cursor and duplicate tracking
pub mod cursor;
// Manual account reallocation helper
pub mod realloc;
// system_program helper module
pub mod system_program;

// Re-export all macros from naclac-macros
pub use naclac_macros::*;
