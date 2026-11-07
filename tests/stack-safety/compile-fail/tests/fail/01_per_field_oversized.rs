// Site: derive/src/accounts/stack_guard.rs — a single data-carrying field
// whose real size_of::<Account<T>>() exceeds the 300-byte per-field budget.
// [u8; 300] alone guarantees this regardless of AccountInfo's own overhead.
use naclac_lang::prelude::*;

#[account]
pub struct BigData {
    pub payload: [u8; 300],
}

#[derive(Accounts)]
pub struct Foo {
    pub target: Account<BigData>,
}

fn main() {}
