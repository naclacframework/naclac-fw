use naclac_lang::prelude::*;

#[component]
pub struct Config {
    pub admin: Address,
    pub fee_bps: u16,
}
