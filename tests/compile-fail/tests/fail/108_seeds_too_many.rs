#![allow(unexpected_cfgs)]
// A PDA takes at most 16 seeds, so a longer `seeds` list is rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"a", b"b", b"c", b"d", b"e", b"f", b"g", b"h", b"i", b"j", b"k", b"l", b"m", b"n", b"o", b"p", b"q"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
