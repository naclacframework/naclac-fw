//! # SPL Token & Token-2022 CPI Abstractions
//!
//! Provides a unified interface for interacting with SPL Token programs across both
//! standard Solana and Pinocchio, using borrow-checked CPI handles. One file per
//! token instruction; the account layouts, the `*Accounts` bundles and the
//! `TokenCpi` trait live in their own files. Everything is re-exported here so
//! public paths stay `naclac_token::token::*`.

mod accounts;
mod approve;
mod burn;
mod close_account;
mod freeze_account;
mod initialize_account;
mod initialize_mint;
#[cfg(feature = "pinocchio")]
mod invoke;
mod layout;
mod mint_to;
mod revoke;
mod set_authority;
mod sync_native;
mod thaw_account;
mod token_cpi;
mod transfer_checked;

pub use {
    accounts::*, approve::*, burn::*, close_account::*, freeze_account::*, initialize_account::*,
    initialize_mint::*, layout::*, mint_to::*, revoke::*, set_authority::*, sync_native::*,
    thaw_account::*, token_cpi::*, transfer_checked::*,
};
