use naclac_lang::prelude::*;

#[defined_type]
pub enum Action {
    Push { items: FixedVec<u8, 4> },
}

fn main() {}
