//! Account constraints provided by a crate other than the derive. A crate
//! that defines an account type can give it `#[account(ns::key = value)]`
//! constraints by implementing [`AccountConstraint`] and exporting a
//! [`ConstraintRules`] table; `#[derive(Accounts)]` calls them after the
//! field has been loaded and validated, so a constraint can only add checks,
//! never remove one. A constraint compares the field's own data with other
//! accounts' addresses or fixed values: checks on what an instruction does
//! with the data belong in the handler.

use crate::{error::NaclacError, prelude::Address, wrappers::ToAddress};

/// A constraint attached to a field of type `A`.
pub trait AccountConstraint<A> {
    /// The values written in the attribute, one entry per key.
    type Args<'a>;

    /// Returns an error when `account` does not satisfy the constraint.
    /// There is no default: a constraint that does not check cannot compile.
    fn check(account: &A, args: &Self::Args<'_>) -> core::result::Result<(), NaclacError>;
}

/// Turns the value written for a key, taken by reference, into the type of
/// that key's `Args` field. Anything with an address (another field of the
/// struct, an `Address`, a reference to one) becomes an `Address`; integers
/// and `bool` are copied. A crate that uses its own type for a key implements
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
