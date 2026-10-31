use naclac_lang::prelude::*;

#[account]
pub struct Ledger {
    pub bump: u8,
    pub value: u64,
}
