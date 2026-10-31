use naclac_lang::prelude::*;

#[account]
pub struct Entry {
    pub bump: u8,
    pub value: u64,
}
