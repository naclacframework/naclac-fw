use naclac_lang::prelude::*;

#[event]
pub struct CounterIncremented {
    pub new_count: u64,
    pub timestamp: i64,
}

#[event]
pub struct CounterDecremented {
    pub new_count: u64,
    pub timestamp: i64,
}
