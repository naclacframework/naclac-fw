use naclac_lang::prelude::*;

#[account]
pub struct TaggedChild {
    pub bump: u8,
    pub value: u64,
}
