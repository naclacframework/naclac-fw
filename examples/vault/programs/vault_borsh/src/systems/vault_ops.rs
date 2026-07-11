use naclac_lang::prelude::*;
use crate::components::vault::Vault;
use crate::components::user_account::UserAccount;
use crate::errors::VaultError;

#[system]
pub fn process_deposit(vault: &mut Vault, user: &mut UserAccount, amount: u64) -> Result<()> {
    vault.total_deposited += amount;
    user.balance += amount;
    Ok(())
}

#[system]
pub fn process_withdraw(vault: &mut Vault, user: &mut UserAccount, amount: u64) -> Result<()> {
    if user.balance < amount {
        return Err(VaultError::InsufficientFunds.into());
    }
    vault.total_deposited -= amount;
    user.balance -= amount;
    Ok(())
}
