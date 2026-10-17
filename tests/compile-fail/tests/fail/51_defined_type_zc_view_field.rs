use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub limit: u64,
    pub tags: ZcVec<u8>,
}

fn main() {}
