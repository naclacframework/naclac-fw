use naclac_lang::prelude::*;

#[component]
pub struct Vault {
    pub total_deposited: u64,
    pub authority: Address,
}
