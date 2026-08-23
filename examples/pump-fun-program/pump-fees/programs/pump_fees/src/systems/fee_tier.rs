use naclac_lang::prelude::*;
use crate::components::{Fees, FeeTier};
use crate::constants::MAX_FEE_TIERS;
use crate::errors::FeesError;

// fee-04 §3: `tiers` must be pre-sorted ascending by
// `market_cap_lamports_threshold` (enforced at config-write time, not here).
// Below the lowest threshold, the lowest tier's fees apply; otherwise the
// highest tier whose threshold the market cap has reached or exceeded wins.
// An empty slice means `fee_config` was never populated (`NoFeeTiers` is
// enforced on every write path) — that's an invariant violation, not a
// "default to something" case, so it errors rather than guessing a value.
#[system]
pub fn calculate_fee_tier(tiers: &[FeeTier], market_cap: u128) -> Result<Fees> {
    let first = tiers.first().ok_or(FeesError::NoFeeTiers)?;

    if market_cap < first.market_cap_lamports_threshold as u128 {
        return Ok(first.fees);
    }

    for tier in tiers.iter().rev() {
        if market_cap >= tier.market_cap_lamports_threshold as u128 {
            return Ok(tier.fees);
        }
    }

    Ok(first.fees)
}

// fees-05 cross-cutting #7: enforce the same caps/order the real program
// does before writing a caller-supplied tier table — non-empty, within
// MAX_FEE_TIERS, strictly ascending by threshold, and every tier's combined
// bps within 10_000 (InvalidFeeTotal must be checked for every tier, not
// just the first/last).
#[system]
pub fn validate_fee_tiers(tiers: &[FeeTier]) -> Result<()> {
    require!(!tiers.is_empty(), FeesError::NoFeeTiers);
    require!(tiers.len() <= MAX_FEE_TIERS, FeesError::TooManyFeeTiers);

    let mut prev_threshold: Option<u64> = None;
    for tier in tiers {
        if let Some(prev) = prev_threshold {
            require!(
                tier.market_cap_lamports_threshold > prev,
                FeesError::FeeTiersNotSorted
            );
        }
        prev_threshold = Some(tier.market_cap_lamports_threshold);

        require!(
            tier_fee_total(&tier.fees)? <= 10_000,
            FeesError::InvalidFeeTotal
        );
    }
    Ok(())
}

#[system(error = "crate::errors::FeesError::MathOverflow")]
fn tier_fee_total(fees: &Fees) -> Result<u64> {
    Ok(fees.lp_fee_bps + fees.protocol_fee_bps + fees.creator_fee_bps)
}
