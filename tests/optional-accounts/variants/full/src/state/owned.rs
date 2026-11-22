use naclac_lang::prelude::*;

#[account]
pub struct Owned {
    pub authority: Address,
    pub value: u64,
}
