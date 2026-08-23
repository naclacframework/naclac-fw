use naclac_lang::prelude::*;

#[component]
pub struct Counter {
    pub bump: u8,
    pub count: u64,
}
