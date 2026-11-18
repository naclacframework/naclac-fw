#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[defined_type]
pub struct Entry {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
}

#[account]
pub struct Book {
    pub first: Entry,
    pub all: [Entry; 2],
}

fn main() {
    let mut book: Book = bytemuck::Zeroable::zeroed();
    assert!(book.first.scores.try_push(1).is_ok());
    assert!(book.all[1].name.try_set("cd").is_ok());
    assert!(book.first.scores.len() == 1 && book.all[1].name.len() == 2);
}
