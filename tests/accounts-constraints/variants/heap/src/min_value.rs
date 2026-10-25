//! A constraint defined outside the derive, used as `#[account(min_value::min = N)]`:
//! the account's `value` must be at least `N`. Exercises the plugin constraint
//! mechanism end to end.

use {crate::components::SeededThing, naclac_lang::prelude::*};

#[derive(Default)]
pub struct Args {
    pub min: Option<u64>,
}

pub struct Constraint;

pub const RULES: ConstraintRules = ConstraintRules {
    rules: &[ConstraintRule::AnyOf {
        keys: &["min"],
        message: "MIN_VALUE_PLUGIN_RULE_MESSAGE: min_value needs a `min` key",
    }],
};

impl AccountConstraint<Account<SeededThing>> for Constraint {
    type Args<'a> = Args;

    fn check(
        account: &Account<SeededThing>,
        args: &Args,
    ) -> core::result::Result<(), NaclacError> {
        match args.min {
            Some(min) if account.value >= min => Ok(()),
            _ => Err(NaclacError::Unauthorized),
        }
    }
}
