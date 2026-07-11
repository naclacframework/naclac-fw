use naclac_lang::prelude::*;
use crate::components::counter::Counter;

#[system]
pub fn process_increment(counter: &mut Counter) -> Result<u64> {
    counter.count += 1;
    Ok(counter.count)
}
