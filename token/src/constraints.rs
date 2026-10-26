//! The `token::`, `mint::` and `associated_token::` account constraints, one
//! module per namespace, each exporting the `Args`, `Constraint` and `RULES`
//! that `#[derive(Accounts)]` calls (see `naclac_core::constraint`). Kept out
//! of every prelude so the module names cannot collide with a program's own.

use crate::prelude::{AccountInfo, Address, NaclacError, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};

/// Binds `$info: AccountInfo` and `$data: &[u8]` (the account's raw data) for
/// the statements that follow, in the calling function (a borrow failure
/// returns from it). The constraints here check the fixed SPL layout on the
/// bytes, so they work on any field whose account data can be read, whatever
/// its declared type.
macro_rules! account_data {
    ($account:expr => $info:ident, $data:ident) => {
        let $info = $crate::wrappers::ToAccountInfo::to_account_info($account);
        #[cfg(not(feature = "pinocchio"))]
        let borrowed = $info
            .try_borrow_data()
            .map_err(|_| $crate::prelude::NaclacError::AccountBorrowFailed)?;
        #[cfg(not(feature = "pinocchio"))]
        let $data: &[u8] = &borrowed;
        #[cfg(feature = "pinocchio")]
        let $data = $info.data();
    };
}

/// Errors unless `info` is owned by the Token or Token-2022 program. The
/// layout checks read raw bytes, which prove nothing about an account any
/// other program could have written.
#[inline(always)]
fn require_token_program_owner(info: &AccountInfo) -> core::result::Result<(), NaclacError> {
    let owner = crate::prelude::Owner::program_owner(info);
    if owner != TOKEN_PROGRAM_ID && owner != TOKEN_2022_PROGRAM_ID {
        return Err(NaclacError::ConstraintOwner);
    }
    Ok(())
}

/// The address of `token_program`, which must be the Token or Token-2022
/// program: the owner of an account created for one of these namespaces.
#[inline(always)]
fn token_program_address(
    token_program: Option<&AccountInfo>,
) -> core::result::Result<Address, NaclacError> {
    let Some(token_program) = token_program else {
        return Err(NaclacError::ProgramIdMismatch);
    };
    let address = crate::wrappers::ToAddress::address(token_program);
    if address != TOKEN_PROGRAM_ID && address != TOKEN_2022_PROGRAM_ID {
        return Err(NaclacError::ProgramIdMismatch);
    }
    Ok(address)
}

/// `token::mint` / `token::authority` on a token account: its stored mint and
/// owner must equal the given accounts. `token_program` is the program a
/// created account is assigned to. An account whose type does not already
/// prove its owner must be owned by the Token or Token-2022 program.
pub mod token {
    use {
        super::{require_token_program_owner, token_program_address},
        crate::{
            prelude::{AccountInfo, Address, CpiHandle, CpiHandleMut, NaclacError},
            token::{initialize_account_with_owner_signed, TokenAccount},
            wrappers::{NaclacAccount, ToAccountInfo, ToAddress},
        },
        naclac_core::{
            constraint::{AccountConstraint, AccountConstraintInit, ConstraintRules},
            constraint_args,
        },
    };

    #[constraint_args]
    #[derive(Default)]
    pub struct Args {
        pub mint: Option<AccountInfo>,
        pub authority: Option<Address>,
        pub token_program: Option<AccountInfo>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules { rules: &[] };

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraint<A> for Constraint {
        type Args<'a> = Args;

        #[inline(always)]
        fn check(account: &A, args: &Args) -> core::result::Result<(), NaclacError> {
            account_data!(account => info, data);
            if !A::OWNER_VALIDATED {
                require_token_program_owner(&info)?;
            }
            if let Some(mint) = &args.mint {
                TokenAccount::check_mint(data, &ToAddress::address(mint))?;
            }
            if let Some(authority) = &args.authority {
                TokenAccount::check_authority(data, authority)?;
            }
            Ok(())
        }
    }

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraintInit<A> for Constraint {
        #[inline(always)]
        fn space(_args: &Args) -> usize {
            165
        }

        #[inline(always)]
        fn owner(args: &Args) -> core::result::Result<Address, NaclacError> {
            token_program_address(args.token_program.as_ref())
        }

        #[inline(always)]
        fn initialize(
            account: &AccountInfo,
            index: usize,
            args: &Args,
            signer_seeds: &[&[&[u8]]],
        ) -> crate::prelude::Result<()> {
            let (Some(token_program), Some(mint), Some(authority)) =
                (args.token_program, args.mint, &args.authority)
            else {
                return Err(NaclacError::ConstraintAccountIsNone.err(index));
            };
            initialize_account_with_owner_signed(
                CpiHandle {
                    info: token_program,
                    _phantom: core::marker::PhantomData,
                },
                CpiHandleMut {
                    info: *account,
                    _phantom: core::marker::PhantomData,
                },
                CpiHandle {
                    info: mint,
                    _phantom: core::marker::PhantomData,
                },
                authority,
                signer_seeds,
            )
        }
    }
}

/// `mint::decimals` / `mint::authority` / `mint::freeze_authority` on a mint:
/// its stored decimals and authorities must equal the given values (an
/// authority key fails on a mint whose authority is unset). `token_program` is
/// the program a created mint is assigned to. A mint whose type does not
/// already prove its owner must be owned by the Token or Token-2022 program.
pub mod mint {
    use {
        super::{require_token_program_owner, token_program_address},
        crate::{
            prelude::{AccountInfo, Address, CpiHandle, CpiHandleMut, NaclacError},
            token::{initialize_mint_signed, Mint},
            wrappers::{NaclacAccount, ToAccountInfo},
        },
        naclac_core::{
            constraint::{AccountConstraint, AccountConstraintInit, ConstraintRules},
            constraint_args,
        },
    };

    #[constraint_args]
    #[derive(Default)]
    pub struct Args {
        pub decimals: Option<u8>,
        pub authority: Option<Address>,
        pub freeze_authority: Option<Address>,
        pub token_program: Option<AccountInfo>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules { rules: &[] };

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraint<A> for Constraint {
        type Args<'a> = Args;

        #[inline(always)]
        fn check(account: &A, args: &Args) -> core::result::Result<(), NaclacError> {
            account_data!(account => info, data);
            if !A::OWNER_VALIDATED {
                require_token_program_owner(&info)?;
            }
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
        }
    }

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraintInit<A> for Constraint {
        #[inline(always)]
        fn space(_args: &Args) -> usize {
            82
        }

        #[inline(always)]
        fn owner(args: &Args) -> core::result::Result<Address, NaclacError> {
            token_program_address(args.token_program.as_ref())
        }

        #[inline(always)]
        fn initialize(
            account: &AccountInfo,
            index: usize,
            args: &Args,
            signer_seeds: &[&[&[u8]]],
        ) -> crate::prelude::Result<()> {
            let (Some(token_program), Some(decimals), Some(authority)) =
                (args.token_program, args.decimals, &args.authority)
            else {
                return Err(NaclacError::ConstraintAccountIsNone.err(index));
            };
            initialize_mint_signed(
                CpiHandle {
                    info: token_program,
                    _phantom: core::marker::PhantomData,
                },
                CpiHandleMut {
                    info: *account,
                    _phantom: core::marker::PhantomData,
                },
                decimals,
                authority,
                args.freeze_authority.as_ref(),
                signer_seeds,
            )
        }
    }
}

/// `associated_token::mint` / `associated_token::authority` (and optionally
/// `associated_token::bump`) on an associated token account: its stored mint
/// and owner must equal the given accounts, it must be owned by
/// `token_program`, and its address must be the associated token address of
/// (authority, token program, mint), using `bump` when given and otherwise
/// searching for it. A created account is made by the Associated Token
/// program, which derives and checks the address itself.
pub mod associated_token {
    use {
        super::token_program_address,
        crate::{
            associated_token::{
                check_associated_token_address, create, create_idempotent,
                create_idempotent_with_extra_accounts_signed, find_associated_token_bump,
                AtaCpiAccounts,
            },
            prelude::{
                AccountInfo, CpiHandle, CpiHandleMut, NaclacError, Owner,
                ASSOCIATED_TOKEN_PROGRAM_ID,
            },
            token::TokenAccount,
            wrappers::{NaclacAccount, ToAccountInfo, ToAddress},
        },
        naclac_core::{
            constraint::{
                AccountConstraint, AccountConstraintCreate, ConstraintCreation, ConstraintRules,
            },
            constraint_args,
        },
    };

    #[constraint_args]
    #[derive(Default)]
    pub struct Args {
        pub mint: Option<AccountInfo>,
        pub authority: Option<AccountInfo>,
        pub bump: Option<u8>,
        pub token_program: Option<AccountInfo>,
        pub associated_token_program: Option<AccountInfo>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules { rules: &[] };

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraint<A> for Constraint {
        type Args<'a> = Args;

        #[inline(always)]
        fn check(account: &A, args: &Args) -> core::result::Result<(), NaclacError> {
            account_data!(account => info, data);
            let (Some(mint), Some(authority), Some(token_program)) =
                (&args.mint, &args.authority, &args.token_program)
            else {
                return Err(NaclacError::ConstraintAccountIsNone);
            };
            let mint = ToAddress::address(mint);
            let authority = ToAddress::address(authority);
            TokenAccount::check_mint(data, &mint)?;
            TokenAccount::check_authority(data, &authority)?;

            let token_program = ToAddress::address(token_program);
            if Owner::program_owner(&info) != token_program {
                return Err(NaclacError::ConstraintOwner);
            }

            let actual = ToAddress::address(&info);
            if let Some(bump) = args.bump {
                return check_associated_token_address(
                    &actual,
                    &authority,
                    &token_program,
                    &mint,
                    bump,
                );
            }
            if find_associated_token_bump(&actual, &authority, &token_program, &mint).is_none() {
                return Err(NaclacError::ConstraintSeeds);
            }
            Ok(())
        }
    }

    impl<A: ToAccountInfo + NaclacAccount> AccountConstraintCreate<A> for Constraint {
        #[inline(always)]
        fn create(args: &Args, creation: ConstraintCreation<'_>) -> crate::prelude::Result<()> {
            let index = creation.index;
            let (Some(mint), Some(authority), Some(token_program), Some(program)) = (
                args.mint,
                args.authority,
                args.token_program,
                args.associated_token_program,
            ) else {
                return Err(NaclacError::ConstraintAccountIsNone.err(index));
            };
            token_program_address(Some(&token_program)).map_err(|e| e.err(index))?;
            if ToAddress::address(&program) != ASSOCIATED_TOKEN_PROGRAM_ID {
                return Err(NaclacError::ProgramIdMismatch.err(index));
            }

            let program = CpiHandle {
                info: program,
                _phantom: core::marker::PhantomData,
            };
            let accounts = AtaCpiAccounts {
                payer: CpiHandleMut {
                    info: creation.payer,
                    _phantom: core::marker::PhantomData,
                },
                associated_token: CpiHandleMut {
                    info: creation.account,
                    _phantom: core::marker::PhantomData,
                },
                authority: CpiHandle {
                    info: authority,
                    _phantom: core::marker::PhantomData,
                },
                mint: CpiHandle {
                    info: mint,
                    _phantom: core::marker::PhantomData,
                },
                system_program: CpiHandle {
                    info: creation.system_program,
                    _phantom: core::marker::PhantomData,
                },
                token_program: CpiHandle {
                    info: token_program,
                    _phantom: core::marker::PhantomData,
                },
            };
            match creation.extra_accounts {
                Some(extra_accounts) => create_idempotent_with_extra_accounts_signed(
                    program,
                    accounts,
                    extra_accounts,
                    creation.payer_signer_seeds,
                ),
                None if creation.idempotent => create_idempotent(program, accounts),
                None => create(program, accounts),
            }
        }
    }
}
