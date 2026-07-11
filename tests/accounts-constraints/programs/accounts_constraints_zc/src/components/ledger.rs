use naclac_lang::prelude::*;

#[component]
pub struct Ledger {
    pub bump: u8,
    pub value: u64,
}
