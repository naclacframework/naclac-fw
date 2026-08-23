// The exact fixture from tests/fail/01_per_field_oversized.rs, but boxed —
// `size_of::<Box<Account<T>>>()` is just a pointer (8 bytes), so wrapping
// the field clears the per-field assert regardless of T's real size.
use naclac_lang::prelude::*;

#[component]
pub struct BigData {
    pub payload: [u8; 300],
}

#[derive(Accounts)]
pub struct Foo {
    pub target: Box<Account<BigData>>,
}

fn main() {}
