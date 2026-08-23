use naclac_lang::prelude::*;

#[system(error = "crate::errors::FeesError::MathOverflow")]
pub fn accumulate_claimed(total: u64, amount: u64) -> Result<u64> {
    Ok(total + amount)
}
