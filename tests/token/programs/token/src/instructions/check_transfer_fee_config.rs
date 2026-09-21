use naclac_lang::prelude::*;

#[instruction_args]
pub struct CheckTransferFeeConfigArgs {
    pub expected_withheld_amount: u64,
    pub expected_newer_basis_points: u16,
    pub expected_newer_maximum_fee: u64,
    pub current_epoch: u64,
    pub transfer_amount: u64,
    pub expected_fee: u64,
    pub expected_post_fee_amount: u64,
}

/// Reads the `TransferFeeConfig` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` (`naclac-token/src/extensions.rs`)
/// and asserts every field, plus `calculate_fee`/`calculate_post_fee_amount`,
/// matches the expected values passed in — the only way to prove the TLV
/// byte-offset math is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
#[instruction(args: CheckTransferFeeConfigArgs)]
pub struct CheckTransferFeeConfig {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_transfer_fee_config(
    ctx: Context<CheckTransferFeeConfig>,
    args: CheckTransferFeeConfigArgs,
) -> Result {
    let config: TransferFeeConfig = ctx.accounts.mint.get_extension()?;

    let matches = config.withheld_amount() == args.expected_withheld_amount
        && config.newer_transfer_fee_basis_points() == args.expected_newer_basis_points
        && config.newer_transfer_fee_maximum_fee() == args.expected_newer_maximum_fee
        && config.calculate_fee(args.current_epoch, args.transfer_amount) == Some(args.expected_fee)
        && config.calculate_post_fee_amount(args.current_epoch, args.transfer_amount)
            == Some(args.expected_post_fee_amount);

    if !matches {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
