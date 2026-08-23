use naclac_lang::prelude::*;

// fee-04 §1: ceil(amount * bps / 10_000), multiplied in u128 so `amount` up to
// u64::MAX times `bps` up to 10_000 never overflows before the division.
#[system(rounding = "up", error = "crate::errors::FeesError::MathOverflow")]
pub fn fee_amount(amount: u128, basis_points: u64) -> Result<u128> {
    Ok(amount * basis_points as u128 / 10_000)
}

// fee-04 §1: a coin with no creator (`Address::default()`, the all-zero
// sentinel) pays zero creator fee regardless of the configured bps.
#[system]
pub fn creator_fee_amount(creator: &Address, amount: u128, basis_points: u64) -> Result<u128> {
    if *creator == Address::default() {
        Ok(0)
    } else {
        fee_amount(amount, basis_points)
    }
}
