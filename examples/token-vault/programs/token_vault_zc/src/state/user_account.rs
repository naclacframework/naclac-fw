use naclac_lang::prelude::*;

#[account]
pub struct UserAccount {
    pub owner: Address,
    pub balance: u64,
    pub bump: u8,
}
