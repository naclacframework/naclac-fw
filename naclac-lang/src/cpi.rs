// ===========================================================================
// cpi.rs — Dual-backend CPI context
// ===========================================================================

//! # Cross-Program Invocation (CPI) Context
//!
//! Provides the `CpiContext` wrapper used to pass programs, accounts, and PDA signer seeds
//! to external programs. It handles the abstraction difference between standard Solana (requiring
//! `Vec` allocations for remaining accounts) and Pinocchio (strictly zero-allocation slices).

// SOLANA-PROGRAM BACKEND
#[cfg(not(feature = "pinocchio"))]
mod solana_cpi {
    use crate::prelude::Vec;
    use solana_program::account_info::AccountInfo;

    pub struct CpiContext<'a, 'b, 'c, 'info, T> {
        pub program: &'a AccountInfo<'info>,
        pub accounts: T,
        pub remaining_accounts: Vec<AccountInfo<'info>>,
        pub signer_seeds: &'a [&'b [&'c [u8]]],
    }

    impl<'a, 'b, 'c, 'info, T> CpiContext<'a, 'b, 'c, 'info, T> {
        pub fn new(program: &'a AccountInfo<'info>, accounts: T) -> Self {
            Self {
                program,
                accounts,
                remaining_accounts: Vec::new(),
                signer_seeds: &[],
            }
        }

        pub fn new_with_signer(
            program: &'a AccountInfo<'info>,
            accounts: T,
            signer_seeds: &'a [&'b [&'c [u8]]],
        ) -> Self {
            Self {
                program,
                accounts,
                remaining_accounts: Vec::new(),
                signer_seeds,
            }
        }

        pub fn with_remaining_accounts(
            mut self,
            remaining_accounts: Vec<AccountInfo<'info>>,
        ) -> Self {
            self.remaining_accounts = remaining_accounts;
            self
        }
    }
}

// PINOCCHIO BACKEND
#[cfg(feature = "pinocchio")]
mod pinocchio_cpi {
    use crate::prelude::AccountInfo;

    pub struct CpiContext<'a, 'b, 'c, 'info, T> {
        pub program: AccountInfo<'info>,
        pub accounts: T,
        pub remaining_accounts: &'info [AccountInfo<'info>],
        pub signer_seeds: &'a [&'b [&'c [u8]]],
    }

    impl<'a, 'b, 'c, 'info, T> CpiContext<'a, 'b, 'c, 'info, T> {
        pub fn new(program: &AccountInfo<'info>, accounts: T) -> Self {
            Self {
                program: program.clone(),
                accounts,
                remaining_accounts: &[],
                signer_seeds: &[],
            }
        }

        pub fn new_with_signer(
            program: &AccountInfo<'info>,
            accounts: T,
            signer_seeds: &'a [&'b [&'c [u8]]],
        ) -> Self {
            Self {
                program: program.clone(),
                accounts,
                remaining_accounts: &[],
                signer_seeds,
            }
        }

        pub fn with_remaining_accounts(
            mut self,
            remaining_accounts: &'info [AccountInfo<'info>],
        ) -> Self {
            self.remaining_accounts = remaining_accounts;
            self
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
pub use solana_cpi::CpiContext;

#[cfg(feature = "pinocchio")]
pub use pinocchio_cpi::CpiContext;

/// Unified AccountMeta for cross-backend CPI usage.
#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub pubkey: crate::prelude::Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(pubkey: crate::prelude::Pubkey, is_writable: bool) -> Self {
        Self {
            pubkey,
            is_signer: false,
            is_writable,
        }
    }
    pub fn new_readonly(pubkey: crate::prelude::Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }
}

pub trait ToAccountMetas {
    fn to_account_metas(&self) -> crate::prelude::Vec<AccountMeta>;
}

pub trait ToAccountInfos<'info> {
    fn to_account_infos(&self) -> crate::prelude::Vec<crate::prelude::AccountInfo<'info>>;
}
