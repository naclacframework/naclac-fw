#![cfg_attr(any(feature = "pinocchio", feature = "no-std"), no_std)]

pub mod prelude;

pub use naclac_core::*;
pub use naclac_core::{context, cpi, cursor, error, event, system_program, wrappers};

#[cfg(all(feature = "token", not(any(feature = "solana", feature = "pinocchio"))))]
compile_error!("the `token` feature requires `solana` or `pinocchio` to also be enabled");

#[cfg(feature = "token")]
pub use naclac_token::{associated_token, token};

#[allow(unused_imports)]
pub use naclac_macros::*;
