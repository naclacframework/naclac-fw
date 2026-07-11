//! # Naclac Language Library
//!
//! The `naclac-lang` crate provides the runtime abstractions, structural types, and zero-copy
//! pointer management utilities required by the `naclac-macros` engine.
//!
//! ## Execution Mode Abstraction
//! This crate is designed to transparently route underlying SVM operations across three execution environments:
//! 1. **Solana Program + Borsh**: Standard heap-allocated types (`Vec`, `String`) and `solana_program`.
//! 2. **Solana Program + Zero-Copy**: Memory-mapped pointer casting via `Pod` and `Zeroable`, while retaining standard CPI tools.
//! 3. **Pinocchio + Zero-Copy**: Pure `no_std` mode leveraging `pinocchio` and zero-allocation logic (`Span`) exclusively.

// no_std when pinocchio or no-std feature is enabled
#![cfg_attr(any(feature = "pinocchio", feature = "no-std"), no_std)] // ===========================================================================
                                                                     // Extern crate declarations
                                                                     // ===========================================================================

// Solana backend
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_program;
#[cfg(not(feature = "pinocchio"))]
pub extern crate solana_system_interface;

// --- Borsh ---
// Only included in the standard Solana backend (Pinocchio uses zero-copy exclusively).
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub extern crate borsh;

pub extern crate bytemuck;

// Pinocchio backend
#[cfg(feature = "pinocchio")]
pub use pinocchio;
#[cfg(feature = "pinocchio")]
pub use pinocchio_log;
#[cfg(feature = "pinocchio")]
pub use pinocchio_system;

// --- Allocator Support ---
// `alloc` is needed for Vec/String support in standard Borsh programs.
// In Zero-Copy (Pinocchio) mode, we use `Span`s to achieve strict zero-allocation.
#[cfg(all(feature = "no-std", not(feature = "pinocchio")))]
extern crate alloc;

// ===========================================================================
// Modules
// ===========================================================================

pub mod context;
pub mod cpi;
pub mod error;
pub mod event;
pub mod prelude;
pub mod wrappers;

// Account cursor and duplicate tracking
pub mod cursor;
// system_program helper module
pub mod system_program;

// Re-export all macros from naclac-macros
pub use naclac_macros::*;

#[cfg(all(feature = "pinocchio", target_os = "solana"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    crate::msg!("Panic!");
    loop {}
}
