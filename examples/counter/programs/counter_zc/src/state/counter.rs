use {crate::constants::SEED_COUNTER, naclac_lang::prelude::*};

#[account]
#[seeds(SEED_COUNTER)]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}
