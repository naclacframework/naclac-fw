#![allow(unexpected_cfgs)]
// Three limits on a nested collection alias are valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

type Names = Vec<Vec<String>>;

#[account]
pub struct Roster {
    #[max_len(1, 2, 3)]
    pub names: Names,
}

fn main() {}
