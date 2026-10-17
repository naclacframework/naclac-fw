use naclac_lang::prelude::*;

#[event]
pub struct Log {
    pub items: FixedVec<u8, 4>,
}

fn main() {}
