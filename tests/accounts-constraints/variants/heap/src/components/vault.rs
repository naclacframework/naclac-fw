use naclac_lang::prelude::*;

#[component]
pub struct Vault {
    pub bump: u8,
    pub admin: Address,
    pub value: u64,
}
