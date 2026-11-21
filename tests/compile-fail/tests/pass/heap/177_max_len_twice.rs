#![allow(unexpected_cfgs)]
// One `#[max_len(..)]` listing every limit is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Roster {
    #[max_len(4)]
    pub names: Vec<u8>,
}

fn main() {}
