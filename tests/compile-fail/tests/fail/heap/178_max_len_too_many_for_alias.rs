#![allow(unexpected_cfgs)]
// At most 3 `#[max_len]` limits apply to a type written as a name.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

type Names = Vec<Vec<Vec<String>>>;

#[account]
pub struct Roster {
    #[max_len(1, 2, 3, 4)]
    pub names: Names,
}

fn main() {}
