use naclac_lang::prelude::*;
use crate::components::Shareholder;
use crate::constants::MAX_SHAREHOLDERS;
use crate::errors::FeesError;

// CREATOR_FEE_SHARING.md: non-empty, <= MAX_SHAREHOLDERS, no duplicate
// addresses, every share_bps > 0, and the total exactly 10_000 bps.
#[system]
pub fn validate_shareholders(shareholders: &[Shareholder]) -> Result<()> {
    require!(!shareholders.is_empty(), FeesError::NoShareholders);
    require!(shareholders.len() <= MAX_SHAREHOLDERS, FeesError::TooManyShareholders);

    let mut total: u32 = 0;
    for (i, shareholder) in shareholders.iter().enumerate() {
        require!(shareholder.share_bps > 0, FeesError::ZeroShareNotAllowed);
        for other in &shareholders[..i] {
            require!(
                other.address != shareholder.address,
                FeesError::DuplicateShareholder
            );
        }
        total = total
            .checked_add(shareholder.share_bps as u32)
            .ok_or(FeesError::ShareCalculationOverflow)?;
    }
    require!(total == 10_000, FeesError::InvalidShareTotal);

    Ok(())
}
