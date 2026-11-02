use naclac_lang::prelude::*;

#[account]
pub struct UserAccount {
    pub balance: u64,
    pub owner: Address,
    pub bump: u8,
}
