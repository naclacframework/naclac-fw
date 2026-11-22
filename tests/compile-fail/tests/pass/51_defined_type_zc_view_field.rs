#![allow(unexpected_cfgs)]
// A defined type stores plain fixed-size fields.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub limit: u64,
    pub tags: [u8; 4],
}

fn main() {}
