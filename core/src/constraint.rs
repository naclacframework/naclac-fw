//! Account constraints provided by a crate other than the derive. A crate
//! that defines an account type can give it `#[account(ns::key = value)]`
//! constraints by implementing [`AccountConstraint`] and exporting a
//! [`ConstraintRules`] table; `#[derive(Accounts)]` calls them after the
//! field has been loaded and validated, so a constraint can only add checks,
//! never remove one. A constraint compares the field's own data with other
//! accounts' addresses or fixed values: checks on what an instruction does
//! with the data belong in the handler.

use crate::{
    error::NaclacError,
    prelude::{AccountInfo, Address, CpiHandle},
    wrappers::{ToAccountInfo, ToAddress},
};

/// A constraint attached to a field of type `A`.
pub trait AccountConstraint<A> {
    /// The values written in the attribute, one entry per key.
    type Args<'a>;

    /// Returns an error when `account` does not satisfy the constraint.
    /// There is no default: a constraint that does not check cannot compile.
    fn check(account: &A, args: &Self::Args<'_>) -> core::result::Result<(), NaclacError>;
}

/// A constraint that can also create the account, for an `init` /
/// `init_if_needed` field. The derive creates the account itself (existence
/// check, payer, rent, the field's own PDA seeds) with the size and owner
/// given here, then calls `initialize` to write its initial state.
pub trait AccountConstraintInit<A>: AccountConstraint<A> {
    /// Size in bytes of the account to create.
    fn space(args: &Self::Args<'_>) -> usize;

    /// The program that will own the new account. An error means the
    /// arguments do not allow creating it.
    fn owner(args: &Self::Args<'_>) -> core::result::Result<Address, NaclacError>;

    /// Writes the initial state of the freshly created `account`, the field at
    /// `index`. `signer_seeds` are the account's own PDA seeds (empty when it
    /// is not a PDA). Errors from the programs it calls are returned
    /// unchanged; its own errors carry `index`.
    fn initialize(
        account: &AccountInfo,
        index: usize,
        args: &Self::Args<'_>,
        signer_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()>;
}

/// What the derive gives a constraint that creates its account itself, for an
/// `init` / `init_if_needed` field. The derive has already checked that an
/// `init` account is empty and that `system_program` is the system program.
pub struct ConstraintCreation<'a> {
    pub account: AccountInfo,
    /// The field's index, for the constraint's own errors.
    pub index: usize,
    pub payer: AccountInfo,
    pub system_program: AccountInfo,
    /// `init_if_needed`: an account that already exists is left as it is.
    pub idempotent: bool,
    /// The field's `extra_accounts = [..]`, when it has one.
    pub extra_accounts: Option<&'a [CpiHandle<'a>]>,
    /// The payer's own PDA seeds (empty when it is not a PDA); given only
    /// with `extra_accounts`.
    pub payer_signer_seeds: &'a [&'a [&'a [u8]]],
}

/// A constraint whose account is created by the program it calls (instead of
/// by the derive through the system program, as `AccountConstraintInit`
/// does), for an `init` / `init_if_needed` field.
pub trait AccountConstraintCreate<A>: AccountConstraint<A> {
    /// Creates the account. Errors from the programs it calls are returned
    /// unchanged; its own errors carry `creation.index`.
    fn create(
        args: &Self::Args<'_>,
        creation: ConstraintCreation<'_>,
    ) -> crate::prelude::Result<()>;
}

/// Turns the value written for a key, taken by reference, into the type of
/// that key's `Args` field. Anything with an address (another field of the
/// struct, an `Address`, a reference to one) becomes an `Address`; another
/// field of the struct becomes its `AccountInfo` handle; integers and `bool`
/// are copied. A crate that uses its own type for a key implements
/// this for a reference to that type. There is deliberately no conversion
/// between different types, so a value of the wrong type is a compile error.
pub trait ConstraintValue<T> {
    fn into_value(self) -> T;
}

impl<A: ToAddress + ?Sized> ConstraintValue<Address> for &A {
    #[inline(always)]
    fn into_value(self) -> Address {
        self.address()
    }
}

impl<A: ToAccountInfo + ?Sized> ConstraintValue<AccountInfo> for &A {
    #[inline(always)]
    fn into_value(self) -> AccountInfo {
        self.to_account_info()
    }
}

macro_rules! copied_constraint_values {
    ($($t:ty),*) => {
        $(
            impl ConstraintValue<$t> for &$t {
                #[inline(always)]
                fn into_value(self) -> $t {
                    *self
                }
            }
        )*
    };
}

copied_constraint_values!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, bool);

/// Where in an account's life the constraint is being applied.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ConstraintPhase {
    /// The account already exists.
    Check,
    /// The account is being created by `init`/`init_if_needed`.
    Init,
}

/// A relation between the keys written for one constraint. `message` is the
/// compile error shown when the relation is broken.
#[derive(Copy, Clone)]
pub enum ConstraintRule {
    /// Whenever `key` is given, `needs` must be given too.
    Requires {
        key: &'static str,
        needs: &'static str,
        message: &'static str,
    },
    /// Like `Requires`, but only while the account is being created.
    InitRequires {
        key: &'static str,
        needs: &'static str,
        message: &'static str,
    },
    /// At least one of `keys` must be given.
    AnyOf {
        keys: &'static [&'static str],
        message: &'static str,
    },
}

/// The rules a constraint's keys must follow, checked while compiling.
pub struct ConstraintRules {
    pub rules: &'static [ConstraintRule],
}

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn contains(given: &[&str], key: &str) -> bool {
    let mut i = 0;
    while i < given.len() {
        if same(given[i], key) {
            return true;
        }
        i += 1;
    }
    false
}

impl ConstraintRules {
    /// Panics with the broken rule's message unless `given`, the keys written
    /// on the field, follow every rule. Evaluated in a `const` item, the
    /// panic is a compile error and nothing is left in the program.
    pub const fn check(&self, given: &[&str], phase: ConstraintPhase) {
        let mut i = 0;
        while i < self.rules.len() {
            match self.rules[i] {
                ConstraintRule::Requires {
                    key,
                    needs,
                    message,
                } => {
                    if contains(given, key) && !contains(given, needs) {
                        panic!("{}", message);
                    }
                }
                ConstraintRule::InitRequires {
                    key,
                    needs,
                    message,
                } => {
                    let init = matches!(phase, ConstraintPhase::Init);
                    if init && contains(given, key) && !contains(given, needs) {
                        panic!("{}", message);
                    }
                }
                ConstraintRule::AnyOf { keys, message } => {
                    let mut found = false;
                    let mut j = 0;
                    while j < keys.len() {
                        if contains(given, keys[j]) {
                            found = true;
                        }
                        j += 1;
                    }
                    if !found {
                        panic!("{}", message);
                    }
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConstraintPhase, ConstraintRule, ConstraintRules};

    const RULES: ConstraintRules = ConstraintRules {
        rules: &[
            ConstraintRule::Requires {
                key: "mint",
                needs: "owner",
                message: "mint needs owner",
            },
            ConstraintRule::InitRequires {
                key: "mint",
                needs: "authority",
                message: "creating a mint needs authority",
            },
            ConstraintRule::AnyOf {
                keys: &["mint", "decimals"],
                message: "give mint or decimals",
            },
        ],
    };

    #[test]
    fn keys_that_follow_every_rule_pass() {
        RULES.check(&["mint", "owner"], ConstraintPhase::Check);
        RULES.check(&["mint", "owner", "authority"], ConstraintPhase::Init);
        RULES.check(&["decimals"], ConstraintPhase::Init);
    }

    #[test]
    fn init_rules_do_not_apply_to_an_existing_account() {
        RULES.check(&["mint", "owner"], ConstraintPhase::Check);
    }

    #[test]
    #[should_panic(expected = "mint needs owner")]
    fn a_missing_required_key_panics_with_the_rules_message() {
        RULES.check(&["mint"], ConstraintPhase::Check);
    }

    #[test]
    #[should_panic(expected = "creating a mint needs authority")]
    fn a_missing_key_on_init_panics_with_the_rules_message() {
        RULES.check(&["mint", "owner"], ConstraintPhase::Init);
    }

    #[test]
    #[should_panic(expected = "give mint or decimals")]
    fn giving_none_of_the_alternatives_panics() {
        RULES.check(&["owner"], ConstraintPhase::Check);
    }

    #[test]
    fn values_convert_to_the_type_of_their_key() {
        use {super::ConstraintValue, crate::prelude::Address};

        const DECIMALS: u8 = 6;
        let address = Address::new_from_array([7u8; 32]);
        let from_address: Address = ConstraintValue::into_value(&address);
        let literal: u64 = ConstraintValue::into_value(&(1 << 40));
        let constant: u8 = ConstraintValue::into_value(&DECIMALS);
        let flag: bool = ConstraintValue::into_value(&true);

        assert!(from_address == address);
        assert!(literal == 1 << 40);
        assert!(constant == 6);
        assert!(flag);
    }

    #[test]
    fn the_check_runs_while_compiling() {
        const _: () = RULES.check(&["mint", "owner"], ConstraintPhase::Check);
    }
}
