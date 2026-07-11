use naclac_lang::prelude::*;

#[component]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}
