use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;
use crate::events::SetParamsEvent;

#[instruction_args]
pub struct SetParamsArgs {
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
    pub withdraw_authority: Address,
    pub enable_migrate: Bool,
    pub pool_migration_fee: u64,
    pub creator_fee_basis_points: u64,
    pub set_creator_authority: Address,
    pub admin_set_creator_authority: Address,
}

#[derive(Accounts)]
#[instruction(args: SetParamsArgs)]
pub struct SetParams {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

// `remaining_accounts[0]` -> `global.fee_recipient`, `remaining_accounts[1..8]`
// -> `global.fee_recipients` — confirmed exactly against real `pump.so`
// (`reference/fee-tier-probe/src/bin/probe21.rs`); not documented in the
// real IDL's own `args` list at all. Each must be rent-exempt, matching the
// real `ConstraintRentExempt` check.
pub fn set_params(ctx: Context<SetParams>, args: SetParamsArgs) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    require!(ctx.remaining_accounts.len() == 8, PumpError::NotEnoughRemainingAccounts);
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(0)?;
    let mut fee_recipients = [Address::default(); 8];
    for (i, slot) in fee_recipients.iter_mut().enumerate() {
        let account = &ctx.remaining_accounts[i];
        require!(account.lamports() >= rent_exempt_minimum, PumpError::FeeRecipientNotRentExempt);
        *slot = account.address();
    }

    let global = &mut ctx.accounts.global;
    global.initial_virtual_token_reserves = args.initial_virtual_token_reserves;
    global.initial_virtual_sol_reserves = args.initial_virtual_sol_reserves;
    global.initial_real_token_reserves = args.initial_real_token_reserves;
    global.token_total_supply = args.token_total_supply;
    global.fee_basis_points = args.fee_basis_points;
    global.withdraw_authority = args.withdraw_authority;
    global.enable_migrate = args.enable_migrate;
    global.pool_migration_fee = args.pool_migration_fee;
    global.creator_fee_basis_points = args.creator_fee_basis_points;
    global.set_creator_authority = args.set_creator_authority;
    global.admin_set_creator_authority = args.admin_set_creator_authority;
    global.fee_recipient = fee_recipients[0];
    global.fee_recipients = fee_recipients[1..8].try_into().unwrap();

    let timestamp = unix_timestamp()?;
    emit!(SetParamsEvent {
        initial_virtual_token_reserves: args.initial_virtual_token_reserves,
        initial_virtual_sol_reserves: args.initial_virtual_sol_reserves,
        initial_real_token_reserves: args.initial_real_token_reserves,
        final_real_sol_reserves: 0u64,
        token_total_supply: args.token_total_supply,
        fee_basis_points: args.fee_basis_points,
        withdraw_authority: args.withdraw_authority,
        enable_migrate: args.enable_migrate,
        pool_migration_fee: args.pool_migration_fee,
        creator_fee_basis_points: args.creator_fee_basis_points,
        fee_recipients,
        timestamp,
        set_creator_authority: args.set_creator_authority,
        admin_set_creator_authority: args.admin_set_creator_authority,
    });

    msg!("Params successfully updated");
    Ok(())
}
