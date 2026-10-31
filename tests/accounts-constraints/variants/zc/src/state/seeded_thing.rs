use naclac_lang::prelude::*;

#[account]
pub struct SeededThing {
    pub bump: u8,
    pub value: u64,
}
