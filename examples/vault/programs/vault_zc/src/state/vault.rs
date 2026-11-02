use naclac_lang::prelude::*;

#[account]
pub struct Vault {
    pub total_deposited: u64,
    pub authority: Address,
}
