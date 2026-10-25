//! The `token::`, `mint::` and `associated_token::` account constraints, one
//! module per namespace, each exporting the `Args`, `Constraint` and `RULES`
//! that `#[derive(Accounts)]` calls (see `naclac_core::constraint`). Kept out
//! of every prelude so the module names cannot collide with a program's own.

use crate::{prelude::NaclacError, wrappers::ToAccountInfo};

/// Runs `check` on the account's raw data. The constraints here check the
/// fixed SPL layout on the bytes, so they work on any field whose account data
/// can be read, whatever its declared type.
#[inline(always)]
fn check_data<A: ToAccountInfo>(
    account: &A,
    check: impl FnOnce(&[u8]) -> core::result::Result<(), NaclacError>,
) -> core::result::Result<(), NaclacError> {
    let info = account.to_account_info();
    #[cfg(not(feature = "pinocchio"))]
    let borrowed = info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed)?;
    #[cfg(not(feature = "pinocchio"))]
    let data: &[u8] = &borrowed;
    #[cfg(feature = "pinocchio")]
    let data = info.data();
    check(data)
}

/// `token::mint` / `token::authority` on an existing token account: its stored
/// mint and owner must equal the given addresses.
pub mod token {
    use {
        super::check_data,
        crate::{
            prelude::{Address, NaclacError},
            token::TokenAccount,
            wrappers::ToAccountInfo,
        },
        naclac_core::constraint::{AccountConstraint, ConstraintRules},
    };

    #[derive(Default)]
    pub struct Args {
        pub mint: Option<Address>,
        pub authority: Option<Address>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules { rules: &[] };

    impl<A: ToAccountInfo> AccountConstraint<A> for Constraint {
        type Args<'a> = Args;

        #[inline(always)]
        fn check(account: &A, args: &Args) -> core::result::Result<(), NaclacError> {
            check_data(account, |data| {
                if let Some(mint) = &args.mint {
                    TokenAccount::check_mint(data, mint)?;
                }
                if let Some(authority) = &args.authority {
                    TokenAccount::check_authority(data, authority)?;
                }
                Ok(())
            })
        }
    }
}

/// `mint::decimals` / `mint::authority` / `mint::freeze_authority` on an
/// existing mint: its stored decimals and authorities must equal the given
/// values. An authority key fails on a mint whose authority is unset.
pub mod mint {
    use {
        super::check_data,
        crate::{
            prelude::{Address, NaclacError},
            token::Mint,
            wrappers::ToAccountInfo,
        },
        naclac_core::constraint::{AccountConstraint, ConstraintRules},
    };

    #[derive(Default)]
    pub struct Args {
        pub decimals: Option<u8>,
        pub authority: Option<Address>,
        pub freeze_authority: Option<Address>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules { rules: &[] };

    impl<A: ToAccountInfo> AccountConstraint<A> for Constraint {
        type Args<'a> = Args;

        #[inline(always)]
        fn check(account: &A, args: &Args) -> core::result::Result<(), NaclacError> {
            check_data(account, |data| {
                if let Some(decimals) = args.decimals {
                    Mint::check_decimals(data, decimals)?;
                }
                if let Some(authority) = &args.authority {
                    Mint::check_authority(data, authority)?;
                }
                if let Some(freeze_authority) = &args.freeze_authority {
                    Mint::check_freeze_authority(data, freeze_authority)?;
                }
                Ok(())
            })
        }
    }
}
