//! # Naclac Language Library
//!
//! The `naclac-lang` crate provides the runtime abstractions, structural types, and zero-copy
//! pointer management utilities required by the `naclac-macros` engine.
//!
//! ## Execution Mode Abstraction
//! This crate is designed to transparently route underlying SVM operations across three execution environments:
//! 1. **Solana Program + Borsh**: Standard heap-allocated types (`Vec`, `String`) and `solana_program`.
//! 2. **Solana Program + Zero-Copy**: Memory-mapped pointer casting via `Pod` and `Zeroable`, while retaining standard CPI tools.
//! 3. **Pinocchio + Zero-Copy**: Pure `no_std` mode leveraging `pinocchio` and zero-allocation logic (`ZcVec`) exclusively.

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

// `naclac-macros` picks exactly one of three account-data representations
// (Borsh / Wincode / raw zero-copy) per program, by checking `not(feature =
// "borsh")`/`not(feature = "wincode")` in every macro that generates
// backend-specific code. That check is silent about *why* it picked what it
// picked — if a dependency somewhere in the build graph ever caused both
// `borsh` and `wincode` to be active on the same program crate at once
// (Cargo's feature flags only ever turn *on*, never off, so one dependency
// edge enabling a feature "wins" for the whole build; this already happened
// once by accident this project, for `wincode`/`pinocchio`), every one of
// those macros would silently compile *both* representations into the same
// program instead of refusing to build. This is the one combination that
// check can't catch on its own, so it's enforced here instead, once, for
// every program that depends on this crate.
#[cfg(all(feature = "borsh", feature = "wincode"))]
compile_error!(
    "naclac-core: the `borsh` and `wincode` features are mutually exclusive — a program \
     selects exactly one account-data representation (Borsh, Wincode, or raw zero-copy, the \
     default when neither is active). Both ended up active on this build at once, which \
     usually means a dependency enabled one of them without your program's own Cargo.toml \
     asking for it (check `cargo tree -e features` for which edge pulled it in)."
);

// Every `borsh`-gated item below also excludes `pinocchio`, so `borsh` alone
// silently does nothing under the Pinocchio backend (falls through to the
// zero-copy branch) instead of failing loudly — Pinocchio's `no_std`,
// zero-allocation design doesn't support Borsh's heap-allocated `String`/
// `Vec` decoding, and never will. `wincode` is deliberately not included in
// this guard: it's planned to become available under Pinocchio once its
// integration is complete, so it keeps the same silent-fallback behavior
// until then.
#[cfg(all(feature = "borsh", feature = "pinocchio"))]
compile_error!(
    "naclac-core: the `borsh` feature is not available under the `pinocchio` backend — \
     Pinocchio is `no_std` and zero-allocation, and Borsh's `String`/`Vec` decoding requires \
     heap allocation. Use the `wincode` or default zero-copy representation instead, or drop \
     the `pinocchio` feature to use the standard Solana backend with Borsh."
);

// --- Borsh ---
// Only included in the standard Solana backend (Pinocchio uses zero-copy exclusively).
#[cfg(all(
    feature = "borsh",
    not(feature = "pinocchio"),
    not(feature = "wincode")
))]
pub extern crate borsh;
#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
pub extern crate wincode;

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

pub mod base58;
pub mod context;
pub mod cpi;
#[cfg(not(feature = "pinocchio"))]
pub mod entrypoint;
pub mod error;
pub mod event;
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod panic;
pub mod prelude;
#[cfg(all(
    feature = "wincode",
    not(feature = "pinocchio"),
    not(feature = "borsh")
))]
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
