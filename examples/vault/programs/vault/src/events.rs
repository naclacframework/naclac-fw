use naclac_lang::prelude::*;

#[event]
pub struct FundsDeposited {
    pub user: Address,
    pub amount: u64,
    pub total_vault_balance: u64,
}

#[event]
pub struct FundsWithdrawn {
    pub user: Address,
    pub amount: u64,
    pub total_vault_balance: u64,
}
