use naclac_lang::prelude::*;

#[defined_type] #[repr(u8)]
pub enum Action {
    Push { items: FixedVec<u8, 4> },
}

fn main() {}
