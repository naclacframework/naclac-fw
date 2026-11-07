//! AMM invariant and swap pricing routines. Amounts a user pays round up and
//! amounts a user receives round down, so rounding never favours the user.

use {crate::errors::AmmError, naclac_lang::prelude::*};

pub const FEE_DENOMINATOR: u64 = 10_000;
pub const SWAP_FEE_BPS: u64 = 25;

/// `⌊a · b / c⌋` without an intermediate overflow. Fails with `Overflow` when
/// `c` is zero.
#[system(error = "crate::errors::AmmError::Overflow")]
fn mul_div_floor_wide(a: u64, b: u64, c: u64) -> Result<u128> {
    Ok(u128::from(a) * u128::from(b) / u128::from(c))
}

/// `⌊a · b / c⌋`. Fails with `Overflow` when `c` is zero or the result does
/// not fit a `u64`.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn mul_div_floor(a: u64, b: u64, c: u64) -> Result<u64> {
    let quotient = mul_div_floor_wide(a, b, c)?;
    u64::try_from(quotient).map_err(|_| AmmError::Overflow.into())
}

/// `⌈a · b / c⌉`. Fails with `Overflow` when `c` is zero or the result does
/// not fit a `u64`.
#[system(rounding = "up", error = "crate::errors::AmmError::Overflow")]
pub fn mul_div_ceil(a: u64, b: u64, c: u64) -> Result<u64> {
    let quotient = u128::from(a) * u128::from(b) / u128::from(c);
    u64::try_from(quotient).map_err(|_| AmmError::Overflow.into())
}

/// Shares minted for the first deposit: `⌊√(amount_a · amount_b)⌋`.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn process_initialize_lp_amount(amount_a: u64, amount_b: u64) -> Result<u64> {
    let product = u128::from(amount_a) * u128::from(amount_b);
    let lp_amount = u64::try_from(integer_sqrt(product)).map_err(|_| AmmError::Overflow)?;
    require!(lp_amount > 0, AmmError::ZeroAmount);
    Ok(lp_amount)
}

/// Output of swapping `amount_in` into a pool holding `x` of the input token
/// and `y` of the output token. The fee is taken from the input first; the
/// product of the reserves never falls.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn process_swap_output(x: u64, y: u64, amount_in: u64, fee_bps: u64) -> Result<u64> {
    require!(x > 0 && y > 0, AmmError::InsufficientLiquidity);
    let amount_in_after_fee =
        mul_div_floor(amount_in, FEE_DENOMINATOR - fee_bps, FEE_DENOMINATOR)?;
    mul_div_floor(y, amount_in_after_fee, x + amount_in_after_fee)
}

/// Fails unless the product of the reserves after a swap, which adds
/// `amount_in` to the input reserve `x` and removes `amount_out` from the
/// output reserve `y`, is at least the product before it.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn require_reserve_product_not_lower(
    x: u64,
    y: u64,
    amount_in: u64,
    amount_out: u64,
) -> Result {
    let before = u128::from(x) * u128::from(y);
    let after = u128::from(x + amount_in) * u128::from(y - amount_out);
    require!(after >= before, AmmError::InvariantViolated);
    Ok(())
}

/// For a deposit of at most `max_amount_a` and `max_amount_b` into reserves
/// `x` and `y` with `lp_supply` shares: `(amount_a, amount_b, shares)`, the
/// amounts taken and the shares minted. Shares are the smaller of the two
/// legs' worth, and a zero-share deposit is refused.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn process_add_liquidity_math(
    x: u64,
    y: u64,
    lp_supply: u64,
    max_amount_a: u64,
    max_amount_b: u64,
) -> Result<(u64, u64, u64)> {
    require!(lp_supply > 0 && x > 0 && y > 0, AmmError::InsufficientLiquidity);

    let shares_a = mul_div_floor_wide(max_amount_a, lp_supply, x)?;
    let shares_b = mul_div_floor_wide(max_amount_b, lp_supply, y)?;
    let lp_to_mint = u64::try_from(shares_a.min(shares_b)).map_err(|_| AmmError::Overflow)?;
    require!(lp_to_mint > 0, AmmError::ZeroAmount);

    let deposit_a = mul_div_ceil(lp_to_mint, x, lp_supply)?;
    let deposit_b = mul_div_ceil(lp_to_mint, y, lp_supply)?;
    Ok((deposit_a, deposit_b, lp_to_mint))
}

/// What burning `lp_amount` of `lp_supply` shares takes out of reserves `x`
/// and `y`: `(amount_a, amount_b)`. A burn that returns nothing on either leg
/// is refused.
#[system(error = "crate::errors::AmmError::Overflow")]
pub fn process_remove_liquidity_math(
    x: u64,
    y: u64,
    lp_supply: u64,
    lp_amount: u64,
) -> Result<(u64, u64)> {
    require!(lp_amount > 0, AmmError::ZeroAmount);
    require!(lp_supply > 0 && lp_amount <= lp_supply, AmmError::InsufficientLiquidity);

    let amount_a = mul_div_floor(x, lp_amount, lp_supply)?;
    let amount_b = mul_div_floor(y, lp_amount, lp_supply)?;
    require!(amount_a > 0 && amount_b > 0, AmmError::ZeroAmount);

    Ok((amount_a, amount_b))
}

/// `⌊√val⌋`. Newton's iteration from `2^⌈bits/2⌉`, a power of two at least as
/// large as the root, so each step only moves down until it reaches the root.
fn integer_sqrt(val: u128) -> u128 {
    if val == 0 {
        return 0;
    }
    let bits = 128 - val.leading_zeros();
    let mut result = 1u128 << bits.div_ceil(2);
    loop {
        let next = (result + val / result) / 2;
        if next >= result {
            return result;
        }
        result = next;
    }
}

#[cfg(test)]
mod tests {
    use {super::*, proptest::prelude::*};

    fn exact(a: u64, b: u64, c: u64) -> (u128, u128) {
        let product = u128::from(a) * u128::from(b);
        let c = u128::from(c);
        (product / c, product.div_ceil(c))
    }

    #[test]
    fn matches_the_values_of_the_lifecycle_test() {
        assert_eq!(
            process_initialize_lp_amount(2_000_000_000, 4_000_000_000).unwrap(),
            2_828_427_124
        );
        assert_eq!(
            process_add_liquidity_math(
                2_000_000_000,
                4_000_000_000,
                2_828_427_124,
                500_000_000,
                1_000_000_000
            )
            .unwrap(),
            (500_000_000, 1_000_000_000, 707_106_781)
        );
    }

    #[test]
    fn a_swap_whose_triple_product_exceeds_u128_succeeds() {
        let out = process_swap_output(
            1_000_000_000_000_000_000,
            1_000_000_000_000_000_000,
            100_000_000_000_000_000,
            SWAP_FEE_BPS,
        );
        assert!(out.is_ok());
    }

    #[test]
    fn integer_sqrt_is_exact_on_small_values_and_perfect_squares() {
        for val in 0u128..=1_000 {
            let root = integer_sqrt(val);
            assert!(root * root <= val && (root + 1) * (root + 1) > val, "val {val}");
        }
        for root in [1u128, 2, 3, 1 << 32, (1 << 64) - 1] {
            let square = root * root;
            assert_eq!(integer_sqrt(square), root);
            assert_eq!(integer_sqrt(square - 1), root - 1);
            assert_eq!(integer_sqrt(square + 1), root);
        }
        assert_eq!(integer_sqrt(u128::MAX), u128::from(u64::MAX));
    }

    #[test]
    fn the_reserve_product_check_accepts_a_swap_that_keeps_the_product() {
        assert!(require_reserve_product_not_lower(2_500_000_000, 5_000_000_000, 100_000_000, 191_845_369)
            .is_ok());
        assert!(require_reserve_product_not_lower(1_000, 1_000, 0, 0).is_ok());
    }

    #[test]
    fn the_reserve_product_check_refuses_a_swap_that_lowers_it() {
        assert!(require_reserve_product_not_lower(1_000, 1_000, 100, 100).is_err());
        assert!(require_reserve_product_not_lower(1_000, 1_000, 0, 1).is_err());
    }

    #[test]
    fn a_swap_into_an_empty_pool_is_refused() {
        assert!(process_swap_output(0, 1_000, 10, SWAP_FEE_BPS).is_err());
        assert!(process_swap_output(1_000, 0, 10, SWAP_FEE_BPS).is_err());
    }

    #[test]
    fn a_fee_above_the_denominator_is_refused() {
        assert!(process_swap_output(1_000, 1_000, 10, FEE_DENOMINATOR + 1).is_err());
    }

    #[test]
    fn a_deposit_that_mints_no_shares_is_refused() {
        assert!(process_add_liquidity_math(1_000_000, 1_000_000, 1, 10, 10).is_err());
    }

    #[test]
    fn a_withdrawal_that_returns_nothing_is_refused() {
        assert!(process_remove_liquidity_math(10, 10, 1_000_000, 1).is_err());
    }

    #[test]
    fn a_withdrawal_above_the_supply_is_refused() {
        assert!(process_remove_liquidity_math(10, 10, 100, 101).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 10_000, ..ProptestConfig::default() })]

        #[test]
        fn mul_div_matches_exact_arithmetic(a in any::<u64>(), b in any::<u64>(), c in 1u64..) {
            let (floor, ceil) = exact(a, b, c);
            prop_assert_eq!(mul_div_floor(a, b, c).ok(), u64::try_from(floor).ok());
            prop_assert_eq!(mul_div_ceil(a, b, c).ok(), u64::try_from(ceil).ok());
        }

        #[test]
        fn mul_div_by_zero_is_refused(a in any::<u64>(), b in any::<u64>()) {
            prop_assert!(mul_div_floor(a, b, 0).is_err());
            prop_assert!(mul_div_ceil(a, b, 0).is_err());
        }

        #[test]
        fn integer_sqrt_is_the_floor_square_root(a in any::<u64>(), b in any::<u64>()) {
            let value = u128::from(a) * u128::from(b);
            let root = integer_sqrt(value);
            prop_assert!(root * root <= value);
            let next = root + 1;
            let beyond = match next.checked_mul(next) {
                Some(square) => square > value,
                None => true,
            };
            prop_assert!(beyond);
        }

        #[test]
        fn the_first_deposit_mints_the_geometric_mean(a in 1u64.., b in 1u64..) {
            let shares = process_initialize_lp_amount(a, b).unwrap();
            prop_assert!((1..=a.max(b)).contains(&shares));
        }

        #[test]
        fn a_swap_never_lowers_the_product_of_the_reserves(
            x in 1u64..1 << 62,
            y in 1u64..1 << 62,
            amount_in in 0u64..1 << 62,
            fee_bps in 0u64..FEE_DENOMINATOR,
        ) {
            let out = process_swap_output(x, y, amount_in, fee_bps).unwrap();
            prop_assert!(out < y);
            let before = u128::from(x) * u128::from(y);
            let after = u128::from(x + amount_in) * u128::from(y - out);
            prop_assert!(after >= before);
        }

        #[test]
        fn the_reserve_product_check_accepts_every_computed_swap(
            x in 1u64..1 << 62,
            y in 1u64..1 << 62,
            amount_in in 0u64..1 << 62,
            fee_bps in 0u64..FEE_DENOMINATOR,
        ) {
            let out = process_swap_output(x, y, amount_in, fee_bps).unwrap();
            prop_assert!(require_reserve_product_not_lower(x, y, amount_in, out).is_ok());
        }

        #[test]
        fn swapping_there_and_back_never_profits(
            x in 1u64..1 << 62,
            y in 1u64..1 << 62,
            amount in 0u64..1 << 62,
            fee_bps in 0u64..FEE_DENOMINATOR,
        ) {
            let out = process_swap_output(x, y, amount, fee_bps).unwrap();
            let back = process_swap_output(y - out, x + amount, out, fee_bps).unwrap();
            prop_assert!(back <= amount);
        }

        #[test]
        fn depositing_then_withdrawing_never_profits(
            x in 1u64..1 << 40,
            y in 1u64..1 << 40,
            supply in 1u64..1 << 40,
            max_a in 0u64..1 << 40,
            max_b in 0u64..1 << 40,
        ) {
            if let Ok((paid_a, paid_b, shares)) =
                process_add_liquidity_math(x, y, supply, max_a, max_b)
            {
                prop_assert!(paid_a <= max_a && paid_b <= max_b && shares > 0);
                let new_supply = supply.checked_add(shares);
                prop_assume!(new_supply.is_some());
                if let Ok((out_a, out_b)) = process_remove_liquidity_math(
                    x + paid_a,
                    y + paid_b,
                    new_supply.unwrap(),
                    shares,
                ) {
                    prop_assert!(out_a <= paid_a && out_b <= paid_b);
                }
            }
        }

        #[test]
        fn a_deposit_never_takes_more_than_the_pools_ratio_allows(
            x in 1u64..1 << 40,
            y in 1u64..1 << 40,
            supply in 1u64..1 << 40,
            max_a in 0u64..1 << 40,
            max_b in 0u64..1 << 40,
        ) {
            if let Ok((paid_a, paid_b, shares)) =
                process_add_liquidity_math(x, y, supply, max_a, max_b)
            {
                let owed_a = u128::from(shares) * u128::from(x);
                let owed_b = u128::from(shares) * u128::from(y);
                prop_assert!(u128::from(paid_a) * u128::from(supply) >= owed_a);
                prop_assert!(u128::from(paid_b) * u128::from(supply) >= owed_b);
            }
        }

        #[test]
        fn a_withdrawal_never_exceeds_the_reserves(
            x in any::<u64>(),
            y in any::<u64>(),
            supply in 1u64..,
            lp_amount in any::<u64>(),
        ) {
            let lp_amount = lp_amount.min(supply);
            if let Ok((a, b)) = process_remove_liquidity_math(x, y, supply, lp_amount) {
                prop_assert!(a <= x && b <= y);
            }
        }
    }
}
