use naclac_lang::prelude::*;

#[event]
pub struct Log {
    pub items: FixedVec<u8, 4>,
    pub name: FixedString<8>,
}

#[event(alloc)]
pub struct AllocLog {
    pub items: FixedVec<u64, 4>,
    pub name: FixedString<8>,
    pub note: String,
}

fn main() {}
