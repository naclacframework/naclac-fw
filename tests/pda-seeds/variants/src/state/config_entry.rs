use naclac_lang::prelude::*;

#[account]
pub struct ConfigEntry {
    pub bump: u8,
    pub value: u64,
}
