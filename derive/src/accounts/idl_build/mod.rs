//! The `idl-build` output of `#[derive(Accounts)]`: per-field account
//! metadata (`account_list`). With the feature off it emits nothing.

mod account_list;
#[cfg(feature = "idl-build")]
mod seeds;

pub(super) use account_list::idl_build_accounts_impl;
