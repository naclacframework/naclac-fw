use naclac_lang::prelude::*;

#[account]
pub struct Profile {
    pub owner: Address,
    pub name: ZcString,
}

fn main() {}
