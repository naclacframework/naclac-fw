use naclac_lang::prelude::*;

#[system]
pub fn process_initialize_lp_amount(amount_a: u64, amount_b: u64) -> Result<u64> {
    let product = amount_a as u128 * amount_b as u128;
    let lp_amount = integer_sqrt(product);
    Ok(lp_amount)
}

#[system]
pub fn process_swap_output(x: u64, y: u64, amount_in: u64) -> Result<u64> {
    // Constant product formula with 0.25% fee:
    // dy = (y * dx * 9975) / (x * 10000 + dx * 9975)
    let amount_in_with_fee = amount_in as u128 * 9975;
    let numerator = amount_in_with_fee * y as u128;
    let denominator = x as u128 * 10000 + amount_in_with_fee;
    if denominator == 0 {
        return Err(crate::errors::AmmError::Overflow.into());
    }
    let amount_out = (numerator / denominator) as u64;
    Ok(amount_out)
}

#[system]
pub fn process_add_liquidity_math(
    x: u64,
    y: u64,
    lp_supply: u64,
    max_amount_a: u64,
    max_amount_b: u64,
) -> Result<(u64, u64, u64)> {
    if lp_supply == 0 || x == 0 || y == 0 {
        return Err(crate::errors::AmmError::ZeroAmount.into());
    }

    // Calculate proportional amount of B for max_amount_a
    // amount_b = max_amount_a * y / x
    let amount_b = (max_amount_a as u128 * y as u128 / x as u128) as u64;

    let (deposit_a, deposit_b) = if amount_b <= max_amount_b {
        (max_amount_a, amount_b)
    } else {
        // Calculate proportional amount of A for max_amount_b
        // amount_a = max_amount_b * x / y
        let amount_a = (max_amount_b as u128 * x as u128 / y as u128) as u64;
        (amount_a, max_amount_b)
    };

    require!(deposit_a > 0 && deposit_b > 0, crate::errors::AmmError::ZeroAmount);

    // lp_to_mint = lp_supply * deposit_a / x
    let lp_to_mint = (lp_supply as u128 * deposit_a as u128 / x as u128) as u64;

    Ok((deposit_a, deposit_b, lp_to_mint))
}

#[system]
pub fn process_remove_liquidity_math(
    x: u64,
    y: u64,
    lp_supply: u64,
    lp_amount: u64,
) -> Result<(u64, u64)> {
    require!(lp_supply > 0 && lp_amount > 0, crate::errors::AmmError::ZeroAmount);
    require!(lp_amount <= lp_supply, crate::errors::AmmError::ZeroAmount);

    let amount_a = (x as u128 * lp_amount as u128 / lp_supply as u128) as u64;
    let amount_b = (y as u128 * lp_amount as u128 / lp_supply as u128) as u64;

    Ok((amount_a, amount_b))
}

fn integer_sqrt(val: u128) -> u64 {
    if val == 0 {
        return 0;
    }
    let mut temp = val.div_ceil(2);
    let mut result = val;
    while temp < result {
        result = temp;
        temp = (val / temp + temp) / 2;
    }
    result as u64
}
