use naclac_lang::prelude::*;

#[event]
pub struct Logged {
    pub id: u64,
    pub note: ZcString,
}

fn main() {}
