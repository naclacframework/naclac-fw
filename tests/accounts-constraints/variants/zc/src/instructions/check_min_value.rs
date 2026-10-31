// `min_value::min = 7` on an account whose `value` is 7: a constraint defined
// outside the derive passes.
use {
    crate::{state::SeededThing, min_value},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CheckMinValue {
    #[account(min_value::min = 7)]
    pub seeded: Account<SeededThing>,
}

pub fn check_min_value(_ctx: Context<CheckMinValue>) -> Result {
    Ok(())
}
