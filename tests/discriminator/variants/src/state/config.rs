use naclac_lang::prelude::*;

#[account]
pub struct Config {
    pub admin: Address,
    pub fee_bps: u16,
}
