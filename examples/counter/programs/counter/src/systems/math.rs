use {
    crate::{errors::CounterError, state::counter::Counter},
    naclac_lang::prelude::*,
};

#[system]
pub fn process_increment(counter: &mut Counter) -> Result<u64> {
    counter.count += 1;
    Ok(counter.count)
}

#[system]
pub fn process_decrement(counter: &mut Counter) -> Result<u64> {
    require!(counter.count > 0, CounterError::Underflow);
    counter.count -= 1;
    Ok(counter.count)
}
