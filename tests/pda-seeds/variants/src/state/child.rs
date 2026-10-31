use naclac_lang::prelude::*;

#[account]
pub struct Child {
    pub bump: u8,
    pub value: u64,
}
