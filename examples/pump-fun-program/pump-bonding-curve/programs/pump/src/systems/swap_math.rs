use naclac_lang::prelude::*;
use crate::errors::PumpError;

// Constant-product curve, solved for the gross SOL cost of removing exactly
// `tokens` from `virtual_token_reserves` (a `buy`).
#[system(rounding = "up", error = "PumpError::MathOverflow")]
pub fn gross_sol_for_tokens_buy(
    tokens: u128,
    virtual_sol_reserves: u128,
    virtual_token_reserves: u128,
) -> Result<u128> {
    Ok(tokens * virtual_sol_reserves / (virtual_token_reserves - tokens))
}

// A `sell` moves tokens the other direction (added to `virtual_token_reserves`
// instead of removed), so the denominator sign flips relative to `buy`.
#[system(rounding = "down", error = "PumpError::MathOverflow")]
pub fn gross_sol_for_tokens_sell(
    tokens: u128,
    virtual_sol_reserves: u128,
    virtual_token_reserves: u128,
) -> Result<u128> {
    Ok(tokens * virtual_sol_reserves / (virtual_token_reserves + tokens))
}

#[system(rounding = "up", error = "PumpError::MathOverflow")]
pub fn fee_amount_ceil(amount: u128, basis_points: u64) -> Result<u128> {
    Ok(amount * basis_points as u128 / 10_000)
}

#[system(rounding = "down", error = "PumpError::MathOverflow")]
pub fn fee_amount_floor(amount: u128, basis_points: u64) -> Result<u128> {
    Ok(amount * basis_points as u128 / 10_000)
}

// `bondingCurveMarketCap` from `pump-public-docs/docs/FEE_PROGRAM_README.md` —
// confirmed real (unlike classic `buy`, which passes a hardcoded 0 for this
// same `get_fees` parameter): `buy_exact_sol_in` computes and passes a real,
// nonzero market cap, verified via `reference/fee-tier-probe/src/bin/probe41.rs`
// against real, deployed `pump.so` + `pump_fees.so`.
#[system(rounding = "down", error = "PumpError::MathOverflow")]
pub fn bonding_curve_market_cap_lamports(
    virtual_sol_reserves: u128,
    token_total_supply: u128,
    virtual_token_reserves: u128,
) -> Result<u128> {
    Ok(virtual_sol_reserves * token_total_supply / virtual_token_reserves)
}

// `buy_exact_sol_in`'s real quote formula, step 4 (confirmed against real
// `pump.so` — `docs/plan/bonding-curve-05-batch1-v2-instructions.md`):
// `tokens_out = floor((net_sol - 1) * virtual_token_reserves / (virtual_sol_reserves + net_sol - 1))`.
#[system(rounding = "down", error = "PumpError::MathOverflow")]
pub fn tokens_out_for_net_sol_exact_in(
    net_sol_minus_one: u128,
    virtual_token_reserves: u128,
    virtual_sol_reserves_plus_net_sol_minus_one: u128,
) -> Result<u128> {
    Ok(net_sol_minus_one * virtual_token_reserves / virtual_sol_reserves_plus_net_sol_minus_one)
}
