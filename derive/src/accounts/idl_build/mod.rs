//! The `idl-build` output of `#[derive(Accounts)]`: per-field account
//! metadata (`account_list`) and the `#[account]` types an instruction
//! references (`account_types`). With the feature off both emit nothing.

mod account_list;
mod account_types;
#[cfg(feature = "idl-build")]
mod seeds;

pub(super) use {
    account_list::idl_build_accounts_impl, account_types::idl_insert_account_types_impl,
};
