#![allow(unexpected_cfgs)]
// A constraint defined outside the derive: `#[account(min_value::min = 10)]`
// is resolved through the `min_value` module's `Args`, `Constraint` and `RULES`.
use naclac_lang::prelude::*;

#[component]
pub struct Counter {
    pub value: u64,
}

mod min_value {
    use {super::Counter, naclac_lang::prelude::*};

    #[derive(Default)]
    pub struct Args {
        pub min: Option<u64>,
    }

    pub struct Constraint;

    pub const RULES: ConstraintRules = ConstraintRules {
        rules: &[ConstraintRule::AnyOf {
            keys: &["min"],
            message: "min_value needs a `min` key",
        }],
    };

    impl AccountConstraint<Account<Counter>> for Constraint {
        type Args<'a> = Args;

        fn check(
            account: &Account<Counter>,
            args: &Args,
        ) -> core::result::Result<(), NaclacError> {
            match args.min {
                Some(min) if account.value >= min => Ok(()),
                _ => Err(NaclacError::Unauthorized),
            }
        }
    }
}

#[derive(Accounts)]
pub struct UseMinValue {
    #[account(min_value::min = 10)]
    pub counter: Account<Counter>,
}

fn main() {}
