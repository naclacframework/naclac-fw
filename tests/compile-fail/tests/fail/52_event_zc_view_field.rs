use naclac_lang::prelude::*;

#[event(alloc)]
pub struct Logged {
    pub id: u64,
    pub note: ZcString,
}

fn main() {}
