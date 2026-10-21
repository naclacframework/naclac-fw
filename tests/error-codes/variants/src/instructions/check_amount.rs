use naclac_lang::prelude::*;
use crate::errors::TestError;

#[derive(Accounts)]
pub struct CheckAmount {
    pub payer: Signer,
}

pub fn check_amount(_ctx: Context<CheckAmount>, amount: u64) -> Result {
    require!(amount > 0, TestError::ZeroAmount);
    require!(amount <= 1000, TestError::TooLarge);
    Ok(())
}
