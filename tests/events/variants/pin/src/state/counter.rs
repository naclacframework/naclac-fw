use naclac_lang::prelude::*;

#[account]
pub struct Counter {
    pub bump: u8,
    pub count: u64,
}
