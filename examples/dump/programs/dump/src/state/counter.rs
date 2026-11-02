use naclac_lang::prelude::*;

#[account]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}
