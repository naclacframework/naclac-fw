use naclac_lang::prelude::*;
use pump_fees_client::instructions::{GetFeesCpi, GetFeesCpiAccounts};
use pump_fees_client::{FeesCpi as Fees, PumpFees};
use crate::constants::PUMP_AUTHORITY_SEED;
use crate::errors::PumpError;

pub struct GetFeesParams {
    pub is_pump_pool: Bool,
    pub market_cap_lamports: u128,
    pub trade_size_lamports: u64,
    pub is_new_quote_mint: Bool,
}

// `pump_fees::get_fees` is a `-> Result<Fees>` handler — naclac's `#[program]`
// macro auto-serializes its return value via `set_return_data`, it never
// returns `Fees` directly through the CPI call itself (`GetFeesCpi::get_fees`
// returns `Result<()>`). The caller must read the return-data buffer back out
// after the CPI returns.
pub fn get_fees_via_cpi<'a>(
    pump_fees_program: &Program<PumpFees>,
    accounts: GetFeesCpiAccounts<'a>,
    pump_authority_bump: u8,
    params: GetFeesParams,
) -> Result<Fees> {
    let signer_seeds: &[&[u8]] = &[PUMP_AUTHORITY_SEED, &[pump_authority_bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    pump_fees_program.get_fees_signed(
        accounts,
        params.is_pump_pool,
        params.market_cap_lamports,
        params.trade_size_lamports,
        params.is_new_quote_mint,
        signer,
    )?;

    let return_data = pinocchio::cpi::get_return_data().ok_or(PumpError::MissingFeesReturnData)?;
    let bytes = return_data.as_slice();
    require!(bytes.len() == core::mem::size_of::<Fees>(), PumpError::MissingFeesReturnData);

    Ok(*bytemuck::from_bytes::<Fees>(bytes))
}
