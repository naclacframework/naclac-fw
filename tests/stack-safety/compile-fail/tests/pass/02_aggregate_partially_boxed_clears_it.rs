// The exact fixture from tests/fail/02_aggregate_oversized.rs, but with 5
// of the 9 fields boxed — boxing doesn't need to be all-or-nothing: the
// remaining 4 unboxed fields (~256 bytes each, ~1024 total) fit comfortably
// under the 1700-byte aggregate budget on their own.
use naclac_lang::prelude::*;

#[component]
pub struct MidData {
    pub payload: [u8; 200],
}

#[derive(Accounts)]
pub struct Foo {
    pub a: Box<Account<MidData>>,
    pub b: Box<Account<MidData>>,
    pub c: Box<Account<MidData>>,
    pub d: Box<Account<MidData>>,
    pub e: Box<Account<MidData>>,
    pub f: Account<MidData>,
    pub g: Account<MidData>,
    pub h: Account<MidData>,
    pub i: Account<MidData>,
}

fn main() {}
