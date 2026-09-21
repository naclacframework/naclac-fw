use naclac_lang::prelude::*;

#[event]
pub struct CounterIncremented {
    pub new_count: u64,
    pub timestamp: i64,
}
