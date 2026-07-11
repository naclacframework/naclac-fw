use naclac_lang::prelude::*;
use crate::components::user_account::UserAccount;
use crate::errors::VaultError;

#[system]
pub fn process_deposit(
    user: &mut UserAccount,
    amount: u64,
) -> Result {
    user.balance = user.balance.checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
        
    Ok(())
}

#[system]
pub fn process_withdraw(
    user: &mut UserAccount,
    amount: u64,
) -> Result {
    if user.balance < amount {
        return Err(VaultError::InsufficientFunds.into());
    }

    user.balance = user.balance.checked_sub(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    Ok(())
}
