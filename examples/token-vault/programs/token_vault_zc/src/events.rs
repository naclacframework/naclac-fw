use naclac_lang::prelude::*;

#[event]
pub struct TokenDeposited {
    pub user: Address,
    pub amount: u64,
    pub total_vault_balance: u64,
}

#[event]
pub struct TokenWithdrawn {
    pub user: Address,
    pub amount: u64,
    pub total_vault_balance: u64,
}
