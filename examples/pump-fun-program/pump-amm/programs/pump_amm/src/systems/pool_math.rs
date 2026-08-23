use naclac_lang::prelude::*;
use crate::constants::LP_BOOTSTRAP_WITHHELD;
use crate::errors::PumpAmmError;

/// Newton's method integer square root — converges to `floor(sqrt(n))` for
/// any `n`, standard algorithm (no external crate needed in a `no_std`
/// program).
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// `initial_liquidity = floor(sqrt(base_amount_in * quote_amount_in))`,
/// `lp_token_amount_out = initial_liquidity - 100` (the withheld amount),
/// all of `lp_token_amount_out` credited to the depositor — confirmed
/// against real bytecode (`reference/fee-tier-probe/src/bin/probe14.rs`:
/// 1e9/2e9 inputs -> minted 1,414,213,462 = floor(sqrt(2e18)) - 100 exactly).
/// Both values are returned since the real `CreatePoolEvent` reports them
/// separately (`initial_liquidity`/`lp_token_amount_out`), not just the
/// final minted amount.
///
/// Validation order and boundaries match the real program exactly
/// (`reference/fee-tier-probe/src/bin/probe37.rs`): `base_amount_in == 0` is
/// rejected before `quote_amount_in == 0` (so a call with both zero reports
/// `ZeroBaseAmount`, not `ZeroQuoteAmount`), and `initial_liquidity ==
/// LP_BOOTSTRAP_WITHHELD` exactly is accepted with `lp_token_amount_out = 0`
/// — only `initial_liquidity < LP_BOOTSTRAP_WITHHELD` is rejected.
pub fn lp_bootstrap_amount(base_amount_in: u64, quote_amount_in: u64) -> Result<(u64, u64)> {
    require!(base_amount_in != 0, PumpAmmError::ZeroBaseAmount);
    require!(quote_amount_in != 0, PumpAmmError::ZeroQuoteAmount);

    let product = (base_amount_in as u128)
        .checked_mul(quote_amount_in as u128)
        .ok_or(PumpAmmError::MathOverflow)?;
    let initial_liquidity = isqrt(product);
    require!(initial_liquidity >= LP_BOOTSTRAP_WITHHELD, PumpAmmError::TooLittlePoolTokenLiquidity);
    let lp_token_amount_out = initial_liquidity - LP_BOOTSTRAP_WITHHELD;

    Ok((
        u64::try_from(initial_liquidity).map_err(|_| PumpAmmError::MathOverflow)?,
        u64::try_from(lp_token_amount_out).map_err(|_| PumpAmmError::MathOverflow)?,
    ))
}
