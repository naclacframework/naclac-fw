#![allow(unexpected_cfgs)]
// A field takes one `#[max_len(..)]`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Roster {
    #[max_len(4)]
    #[max_len(8)]
    pub names: Vec<u8>,
}

fn main() {}
