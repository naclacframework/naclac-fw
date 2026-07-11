use naclac_lang::prelude::*;

#[component]
pub struct UserAccount {
    pub owner: Address,
    pub balance: u64,
    pub bump: u8,
}
