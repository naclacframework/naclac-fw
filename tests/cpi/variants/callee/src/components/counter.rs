use naclac_lang::prelude::*;

#[component]
pub struct Counter {
    pub bump: u8,
    pub value: u64,
    // The only account allowed to call `authorized_increment` — set at
    // init time to whatever address the caller passes in (in the real
    // cross-program CPI test, this is `cpi_caller`'s own PDA authority).
    pub authority: Address,
}
