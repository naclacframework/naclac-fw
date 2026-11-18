//! Fixed-point accuracy, time, conviction, and stake-weight scoring.

use {crate::errors::CustomError, naclac_lang::prelude::*};

pub const MATH_PRECISION: u128 = 1_000_000;

/// Returns inverse-relative-error accuracy, or zero for a zero result or a
/// nonzero accuracy buffer reached by the absolute prediction error.
pub fn calculate_accuracy_score(
    prediction: u64,
    result: u64,
    max_accuracy_buffer: u64,
) -> Result<u64> {
    if result == 0 {
        return Ok(0);
    }

    let diff = if prediction > result {
        prediction - result
    } else {
        result - prediction
    };

    if max_accuracy_buffer > 0 && diff >= max_accuracy_buffer {
        return Ok(0);
    }

    let error_scaled = (diff as u128)
        .checked_mul(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(result as u128)
        .ok_or(CustomError::MathOverflow)?;

    let denominator = MATH_PRECISION
        .checked_add(
            error_scaled
                .checked_mul(10)
                .ok_or(CustomError::MathOverflow)?,
        )
        .ok_or(CustomError::MathOverflow)?;

    let score = (MATH_PRECISION * MATH_PRECISION)
        .checked_div(denominator)
        .ok_or(CustomError::MathOverflow)?;

    score_to_u64(score)
}

/// Returns the fixed-point quadratic early-entry multiplier, with a unit
/// multiplier at or after cutoff. Rejects nonpositive scoring periods,
/// entries before the start, and arithmetic or result-conversion overflow.
pub fn calculate_time_bonus(start_time: i64, cutoff_time: i64, entry_time: i64) -> Result<u64> {
    if cutoff_time <= start_time || entry_time < start_time {
        return Err(CustomError::InvalidTime.into());
    }

    let total_duration = cutoff_time
        .checked_sub(start_time)
        .ok_or(CustomError::MathOverflow)? as u128;

    if entry_time >= cutoff_time {
        return Ok(MATH_PRECISION as u64);
    }

    let time_from_start = cutoff_time
        .checked_sub(entry_time)
        .ok_or(CustomError::MathOverflow)? as u128;

    let t_scaled = time_from_start
        .checked_mul(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(total_duration)
        .ok_or(CustomError::MathOverflow)?;

    let t_sq_scaled = t_scaled
        .checked_mul(t_scaled)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?;

    let bonus = t_sq_scaled
        .checked_mul(15)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(10)
        .ok_or(CustomError::MathOverflow)?;

    let factor = MATH_PRECISION
        .checked_add(bonus)
        .ok_or(CustomError::MathOverflow)?;

    score_to_u64(factor)
}

/// Converts a scoring result to its public representation without truncation.
fn score_to_u64(score: u128) -> Result<u64> {
    u64::try_from(score).map_err(|_| CustomError::MathOverflow.into())
}

/// Returns a 1.5x fixed-point multiplier for an unchanged prediction, otherwise 1x.
pub fn calculate_conviction_bonus(update_count: u32) -> u64 {
    if update_count == 0 {
        1_500_000
    } else {
        1_000_000
    }
}

/// Returns stake multiplied by the three fixed-point scoring factors, truncating
/// to an integer weight; an overflowing intermediate product is an error.
pub fn calculate_weight(
    stake: u64,
    accuracy_score_scaled: u64,
    time_bonus_scaled: u64,
    conviction_scaled: u64,
) -> Result<u128> {
    let raw_product = (stake as u128)
        .checked_mul(accuracy_score_scaled as u128)
        .ok_or(CustomError::MathOverflow)?
        .checked_mul(time_bonus_scaled as u128)
        .ok_or(CustomError::MathOverflow)?
        .checked_mul(conviction_scaled as u128)
        .ok_or(CustomError::MathOverflow)?;

    let final_weight = raw_product
        .checked_div(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?
        .checked_div(MATH_PRECISION)
        .ok_or(CustomError::MathOverflow)?;

    Ok(final_weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accuracy_zero_result_always_scores_zero() {
        for prediction in [0, 1, u64::MAX] {
            for buffer in [0, 1, u64::MAX] {
                assert_eq!(calculate_accuracy_score(prediction, 0, buffer), Ok(0));
            }
        }
    }

    #[test]
    fn accuracy_exact_matches_score_full_precision() {
        for result in [1, 100, u64::MAX] {
            for buffer in [0, 1, u64::MAX] {
                assert_eq!(
                    calculate_accuracy_score(result, result, buffer),
                    Ok(1_000_000)
                );
            }
        }
    }

    #[test]
    fn accuracy_exact_outputs_preserve_rounding() {
        for (prediction, result, expected) in [
            (90, 100, 500_000),
            (110, 100, 500_000),
            (0, 100, 90_909),
            (200, 100, 90_909),
            (101, 100, 909_090),
            (101, 103, 837_401),
        ] {
            assert_eq!(
                calculate_accuracy_score(prediction, result, 0),
                Ok(expected)
            );
        }
    }

    #[test]
    fn accuracy_buffer_is_exclusive_and_zero_disables_it() {
        for prediction in [90, 110] {
            assert_eq!(calculate_accuracy_score(prediction, 100, 9), Ok(0));
            assert_eq!(calculate_accuracy_score(prediction, 100, 10), Ok(0));
            assert_eq!(calculate_accuracy_score(prediction, 100, 11), Ok(500_000));
            assert_eq!(calculate_accuracy_score(prediction, 100, 0), Ok(500_000));
        }
        assert_eq!(calculate_accuracy_score(99, 100, 1), Ok(0));
        assert_eq!(calculate_accuracy_score(100, 100, 1), Ok(1_000_000));
    }

    #[test]
    fn accuracy_u64_boundaries_do_not_overflow() {
        assert_eq!(calculate_accuracy_score(u64::MAX, 1, 0), Ok(0));
        assert_eq!(calculate_accuracy_score(0, u64::MAX, 0), Ok(90_909));
        assert_eq!(calculate_accuracy_score(0, u64::MAX, u64::MAX), Ok(0));
        assert_eq!(
            calculate_accuracy_score(u64::MAX - 1, u64::MAX, 0),
            Ok(1_000_000)
        );
    }

    #[test]
    fn time_factor_exact_outputs_preserve_rounding() {
        for (entry, expected) in [
            (0, 2_500_000),
            (25, 1_843_750),
            (50, 1_375_000),
            (99, 1_000_150),
            (100, 1_000_000),
            (101, 1_000_000),
        ] {
            assert_eq!(calculate_time_bonus(0, 100, entry), Ok(expected));
        }
        assert_eq!(calculate_time_bonus(10, 13, 11), Ok(1_666_664));
        assert_eq!(calculate_time_bonus(-100, 0, -50), Ok(1_375_000));
    }

    #[test]
    fn time_factor_rejects_entries_before_start() {
        for (start, cutoff, entry) in [
            (0, 100, -100),
            (0, 1, i64::MIN + 2),
            (i64::MIN + 1, -1, i64::MIN),
            (i64::MAX - 1, i64::MAX, i64::MIN),
        ] {
            assert_eq!(
                calculate_time_bonus(start, cutoff, entry),
                Err(CustomError::InvalidTime.into())
            );
        }
    }

    #[test]
    fn time_factor_rejects_zero_and_reversed_durations() {
        for (start, cutoff) in [(5, 5), (10, 5), (i64::MAX, i64::MIN)] {
            for entry in [i64::MIN, 4, 5, 6, i64::MAX] {
                assert_eq!(
                    calculate_time_bonus(start, cutoff, entry),
                    Err(CustomError::InvalidTime.into())
                );
            }
        }
    }

    #[test]
    fn time_factor_at_or_after_valid_cutoff_is_unit() {
        for (start, cutoff, entry) in [
            (0, 1, 1),
            (0, 1, i64::MAX),
            (i64::MIN, -1, -1),
            (i64::MIN, -1, i64::MAX),
            (i64::MAX - 1, i64::MAX, i64::MAX),
        ] {
            assert_eq!(calculate_time_bonus(start, cutoff, entry), Ok(1_000_000));
        }
    }

    #[test]
    fn time_factor_signed_duration_overflow_returns_math_overflow() {
        for (start, cutoff) in [(i64::MIN, i64::MAX), (i64::MIN, 0), (-1, i64::MAX)] {
            for entry in [start, cutoff - 1, cutoff] {
                assert_eq!(
                    calculate_time_bonus(start, cutoff, entry),
                    Err(CustomError::MathOverflow.into())
                );
            }
        }
    }

    #[test]
    fn time_factor_extreme_representable_durations_preserve_outputs() {
        for (start, cutoff) in [(i64::MIN, -1), (0, i64::MAX)] {
            assert_eq!(calculate_time_bonus(start, cutoff, start), Ok(2_500_000));
            assert_eq!(
                calculate_time_bonus(start, cutoff, cutoff - 1),
                Ok(1_000_000)
            );
        }
        assert_eq!(
            calculate_time_bonus(i64::MIN, i64::MIN + 1, i64::MIN),
            Ok(2_500_000)
        );
        assert_eq!(
            calculate_time_bonus(i64::MAX - 1, i64::MAX, i64::MAX - 1),
            Ok(2_500_000)
        );
    }

    #[test]
    fn scoring_result_conversion_rejects_narrowing_overflow() {
        for score in [0, MATH_PRECISION, u64::MAX as u128] {
            assert_eq!(score_to_u64(score), Ok(score as u64));
        }
        for score in [u64::MAX as u128 + 1, u128::MAX] {
            assert_eq!(score_to_u64(score), Err(CustomError::MathOverflow.into()));
        }
    }

    #[test]
    fn conviction_depends_only_on_whether_updates_are_zero() {
        assert_eq!(calculate_conviction_bonus(0), 1_500_000);
        for updates in [1, 2, 1_000, u32::MAX] {
            assert_eq!(calculate_conviction_bonus(updates), 1_000_000);
        }
    }

    #[test]
    fn weight_exact_outputs_preserve_rounding() {
        for (stake, accuracy, time, conviction, expected) in [
            (100, 1_000_000, 1_000_000, 1_000_000, 100),
            (100, 1_000_000, 2_500_000, 1_500_000, 375),
            (1_234_567, 90_909, 1_375_000, 1_500_000, 231_481),
            (1, 999_999, 1_000_000, 1_000_000, 0),
            (123_456_789, 333_333, 1_666_666, 1_500_000, 102_880_513),
        ] {
            assert_eq!(
                calculate_weight(stake, accuracy, time, conviction),
                Ok(expected)
            );
        }
    }

    #[test]
    fn weight_zero_factors_produce_zero_when_intermediates_fit() {
        for (stake, accuracy, time, conviction) in [
            (0, u64::MAX, u64::MAX, u64::MAX),
            (u64::MAX, 0, u64::MAX, u64::MAX),
            (u64::MAX, u64::MAX, 0, u64::MAX),
            (u64::MAX, 1, u64::MAX, 0),
            (0, 0, 0, 0),
        ] {
            assert_eq!(calculate_weight(stake, accuracy, time, conviction), Ok(0));
        }
    }

    #[test]
    fn weight_maximum_stake_and_wide_result_do_not_overflow() {
        assert_eq!(
            calculate_weight(u64::MAX, 1_000_000, 1_000_000, 1_000_000),
            Ok(u64::MAX as u128)
        );
        assert_eq!(
            calculate_weight(u64::MAX, 1_000_000, 2_500_000, 1_500_000),
            Ok((u64::MAX as u128) * 15 / 4)
        );
    }

    #[test]
    fn weight_intermediate_overflows_return_math_overflow() {
        for (stake, accuracy, time, conviction) in [
            (u64::MAX, u64::MAX, 2, 1),
            (u64::MAX, 1, u64::MAX, 2),
            (u64::MAX, u64::MAX, u64::MAX, u64::MAX),
            (u64::MAX, u64::MAX, 2, 0),
        ] {
            assert_eq!(
                calculate_weight(stake, accuracy, time, conviction),
                Err(CustomError::MathOverflow.into())
            );
        }
    }

    #[test]
    fn scoring_pipeline_exact_output() {
        let accuracy = calculate_accuracy_score(90, 100, 0).unwrap();
        let time = calculate_time_bonus(0, 100, 50).unwrap();
        let conviction = calculate_conviction_bonus(0);
        assert_eq!(calculate_weight(100, accuracy, time, conviction), Ok(103));
    }
}
