use naclac_lang::prelude::*;

#[account]
pub struct Vault {
    pub owner: Address,
    pub balance: u64,
}
