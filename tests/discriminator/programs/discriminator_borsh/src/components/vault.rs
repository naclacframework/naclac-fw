use naclac_lang::prelude::*;

#[component]
pub struct Vault {
    pub owner: Address,
    pub balance: u64,
}
