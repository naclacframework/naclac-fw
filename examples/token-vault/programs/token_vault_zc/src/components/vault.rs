use naclac_lang::prelude::*;

#[component]
pub struct Vault {
    pub authority: Address,      
    pub vault_id: u64,
    pub bump: u8,
}