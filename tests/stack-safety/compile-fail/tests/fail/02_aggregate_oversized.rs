// Site: naclac-macros/src/accounts.rs:352 — no single field is over the
// 300-byte per-field budget, but nine ~256-byte fields sum well past the
// 1700-byte aggregate budget. This is the case the per-field check alone
// can't catch.
use naclac_lang::prelude::*;

#[component]
pub struct MidData {
    pub payload: [u8; 200],
}

#[derive(Accounts)]
pub struct Foo {
    pub a: Account<MidData>,
    pub b: Account<MidData>,
    pub c: Account<MidData>,
    pub d: Account<MidData>,
    pub e: Account<MidData>,
    pub f: Account<MidData>,
    pub g: Account<MidData>,
    pub h: Account<MidData>,
    pub i: Account<MidData>,
}

fn main() {}
