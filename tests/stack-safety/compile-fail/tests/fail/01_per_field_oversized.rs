// Site: naclac-macros/src/accounts.rs:332 — a single data-carrying field
// whose real size_of::<Account<T>>() exceeds the 300-byte per-field budget.
// [u8; 300] alone guarantees this regardless of AccountInfo's own overhead.
use naclac_lang::prelude::*;

#[component]
pub struct BigData {
    pub payload: [u8; 300],
}

#[derive(Accounts)]
pub struct Foo {
    pub target: Account<BigData>,
}

fn main() {}
