// `min_value::min = 8` on an account whose `value` is 7: the constraint fails
// with `Unauthorized`.
use {
    crate::{components::SeededThing, min_value},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CheckMinValueUnmet {
    #[account(min_value::min = 8)]
    pub seeded: Account<SeededThing>,
}

pub fn check_min_value_unmet(_ctx: Context<CheckMinValueUnmet>) -> Result {
    Ok(())
}
