use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of every ongoing-action `TransferFeeConfig` CPI in
/// `naclac-token/src/extensions.rs` (`transfer_checked_with_fee`,
/// `harvest_withheld_tokens_to_mint`, `withdraw_withheld_tokens_from_mint`,
/// `withdraw_withheld_tokens_from_accounts`, `set_transfer_fee`) — chains
/// all five against a real Token-2022 mint/vaults and asserts the resulting
/// on-chain balances/withheld amounts/config at each step, not just that
/// Token-2022 accepted the instruction.
#[instruction_args]
pub struct ExerciseTransferFeeCpisArgs {
    pub amount1: u64,
    pub decimals: u8,
    pub fee1: u64,
    pub amount2: u64,
    pub fee2: u64,
    pub new_basis_points: u16,
    pub new_maximum_fee: u64,
}

#[derive(Accounts)]
#[instruction(args: ExerciseTransferFeeCpisArgs)]
pub struct ExerciseTransferFeeCpis {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub source: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub vault_b: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub collector: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

pub fn exercise_transfer_fee_cpis(ctx: Context<ExerciseTransferFeeCpis>, args: ExerciseTransferFeeCpisArgs) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let source_before = ctx.accounts.source.amount();

    transfer_checked_with_fee_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.source.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.vault_b.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        TransferCheckedWithFeeParams {
            amount: args.amount1,
            decimals: args.decimals,
            fee: args.fee1,
        },
        signer,
    )?;

    if ctx.accounts.source.amount() != source_before - args.amount1 {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let net1 = args.amount1 - args.fee1;
    if ctx.accounts.vault_b.amount() != net1 {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let vault_b_fee_ext: TransferFeeAmount = ctx.accounts.vault_b.get_extension()?;
    if vault_b_fee_ext.withheld_amount() != args.fee1 {
        return Err(NaclacError::ConstraintAddress.into());
    }

    harvest_withheld_tokens_to_mint_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        &[ctx.accounts.vault_b.to_cpi_handle()],
        &[],
    )?;

    let mint_fee_config: TransferFeeConfig = ctx.accounts.mint.get_extension()?;
    if mint_fee_config.withheld_amount() != args.fee1 {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let vault_b_fee_ext: TransferFeeAmount = ctx.accounts.vault_b.get_extension()?;
    if vault_b_fee_ext.withheld_amount() != 0 {
        return Err(NaclacError::ConstraintAddress.into());
    }

    withdraw_withheld_tokens_from_mint_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.collector.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        signer,
    )?;

    if ctx.accounts.collector.amount() != args.fee1 {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let mint_fee_config: TransferFeeConfig = ctx.accounts.mint.get_extension()?;
    if mint_fee_config.withheld_amount() != 0 {
        return Err(NaclacError::ConstraintAddress.into());
    }

    let vault_b_before = ctx.accounts.vault_b.amount();
    transfer_checked_with_fee_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.source.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.vault_b.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        TransferCheckedWithFeeParams {
            amount: args.amount2,
            decimals: args.decimals,
            fee: args.fee2,
        },
        signer,
    )?;
    let net2 = args.amount2 - args.fee2;
    if ctx.accounts.vault_b.amount() != vault_b_before + net2 {
        return Err(NaclacError::ConstraintAddress.into());
    }

    withdraw_withheld_tokens_from_accounts_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.collector.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        &[ctx.accounts.vault_b.to_cpi_handle()],
        signer,
    )?;

    if ctx.accounts.collector.amount() != args.fee1 + args.fee2 {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let vault_b_fee_ext: TransferFeeAmount = ctx.accounts.vault_b.get_extension()?;
    if vault_b_fee_ext.withheld_amount() != 0 {
        return Err(NaclacError::ConstraintAddress.into());
    }

    set_transfer_fee_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        args.new_basis_points,
        args.new_maximum_fee,
        signer,
    )?;

    let mint_fee_config: TransferFeeConfig = ctx.accounts.mint.get_extension()?;
    if mint_fee_config.newer_transfer_fee_basis_points() != args.new_basis_points
        || mint_fee_config.newer_transfer_fee_maximum_fee() != args.new_maximum_fee
    {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
